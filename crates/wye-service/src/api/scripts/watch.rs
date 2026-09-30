//! SCR-08: tell open editors when a script file changes on disk.
//!
//! The configuration directory is watched recursively (editors and
//! dotfile managers replace files, and `rules/` may appear later). After
//! 100 ms of quiet a changed script is read; when its text differs from
//! what the service last read or wrote, `ScriptFileChanged(scope)` is
//! emitted. The service's own `SetScript` writes are therefore silent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::Instant;
use wye_api::actions::ScriptScope;

use super::files::ScriptFiles;
use crate::api::link::Environment;
use crate::context::{ServiceContext, blocking};

/// Quiet time before a changed script is read.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// Watch the current environment's scripts until shutdown.
pub(crate) async fn run(ctx: ServiceContext) {
    let mut environments = ctx.environment_changes();
    let shutdown = ctx.until_shutdown();
    tokio::pin!(shutdown);
    loop {
        let environment = environments.borrow_and_update().clone();
        let watching = watch_environment(&ctx, environment);
        tokio::select! {
            () = &mut shutdown => return,
            changed = environments.changed() => {
                if changed.is_err() {
                    return;
                }
            }
            () = watching => return,
        }
    }
}

async fn watch_environment(ctx: &ServiceContext, environment: Option<Arc<Environment>>) {
    let Some(environment) = environment else {
        return std::future::pending().await;
    };
    let files = ScriptFiles::beside(&environment.config);
    let (sender, events) = mpsc::unbounded_channel();
    let Some(_watcher) = start(&files, sender) else {
        // Nothing to watch; `GetScript` and `SetScript` still work.
        return std::future::pending().await;
    };
    announce_changes(ctx, &files, events).await;
}

fn start(
    files: &ScriptFiles,
    sender: mpsc::UnboundedSender<PathBuf>,
) -> Option<RecommendedWatcher> {
    if let Err(error) = std::fs::create_dir_all(files.dir()) {
        tracing::warn!(%error, dir = %files.dir().display(), "cannot create the configuration directory");
        return None;
    }
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            for path in event.paths {
                // The receiver is gone only while the watcher is dropped.
                let _closing = sender.send(path);
            }
        }
    });
    let mut watcher = watcher
        .inspect_err(|error| tracing::warn!(%error, "cannot watch the script files"))
        .ok()?;
    watcher
        .watch(files.dir(), RecursiveMode::Recursive)
        .inspect_err(|error| tracing::warn!(%error, "cannot watch the script files"))
        .ok()?;
    Some(watcher)
}

async fn announce_changes(
    ctx: &ServiceContext,
    files: &ScriptFiles,
    mut events: mpsc::UnboundedReceiver<PathBuf>,
) {
    let mut pending: BTreeMap<String, (ScriptScope, Instant)> = BTreeMap::new();
    loop {
        let next = pending.values().map(|(_, deadline)| *deadline).min();
        let sleep = async {
            match next {
                Some(deadline) => tokio::time::sleep_until(deadline).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            path = events.recv() => {
                let Some(path) = path else { return };
                for scope in scopes_behind(files, &path) {
                    pending.entry(scope.to_string()).or_insert((scope, Instant::now() + DEBOUNCE));
                }
            }
            () = sleep => {
                let now = Instant::now();
                let (due, waiting): (BTreeMap<_, _>, BTreeMap<_, _>) = pending
                    .into_iter()
                    .partition(|(_, (_, deadline))| *deadline <= now);
                pending = waiting;
                for (scope, _) in due.into_values() {
                    announce(ctx, files, scope).await;
                }
            }
        }
    }
}

/// The scripts an event at `path` may have changed. A new `rules/`
/// directory counts for every script in it: files written right after the
/// directory appears can land before the watcher covers it.
fn scopes_behind(files: &ScriptFiles, path: &Path) -> Vec<ScriptScope> {
    match files.scope_of(path) {
        Some(scope) => vec![scope],
        None if path == files.rules_dir() => files.rule_scripts(),
        None => Vec::new(),
    }
}

/// Emit `ScriptFileChanged` when `scope`'s text is not what the service
/// last saw.
async fn announce(ctx: &ServiceContext, files: &ScriptFiles, scope: ScriptScope) {
    let reading = files.clone();
    let read_scope = scope.clone();
    let text = match blocking(move || reading.read(&read_scope)).await {
        Ok(Ok(text)) => text,
        Ok(Err(error)) | Err(error) => {
            tracing::warn!(%error, %scope, "cannot read the changed script");
            return;
        }
    };
    if !ctx.scripts().saw(&scope, text.as_deref()) {
        return;
    }
    tracing::info!(%scope, "script changed on disk");
    if let Err(error) = crate::bus::script_file_changed(ctx, &scope.to_string()).await {
        tracing::warn!(%error, %scope, "cannot announce the changed script");
    }
}
