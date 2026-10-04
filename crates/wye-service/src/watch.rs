//! File watchers: the configuration directory (reload, SET-06),
//! `mimeapps.list` and `kdeglobals` (DEF-03), application directories and
//! browser profile files (DISC-02), and the browsers' top-level directories
//! in the home and configuration directories (extension host manifests,
//! BEXT-04).
//!
//! One notify watcher per environment, rebuilt when the environment
//! changes, a watched directory appears or a profile symlink moves. Events
//! are debounced per kind: 100 ms for the configuration, 2 s for a new
//! browser directory, 500 ms for the rest, so an editor's save or a package
//! switch is handled once.

mod plan;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use self::plan::{Kind, Plan};
use crate::api;
use crate::api::link::Environment;
use crate::context::{ServiceContext, blocking};

/// Quiet time before the configuration is reloaded.
const CONFIG_DEBOUNCE: Duration = Duration::from_millis(100);
/// Quiet time before apps are rescanned or the registration is checked.
const INVENTORY_DEBOUNCE: Duration = Duration::from_millis(500);
/// Quiet time before the extension's host manifests are written for a new
/// browser directory: a browser creates `~/.config/BraveSoftware` and then
/// `Brave-Browser` in it, or `~/.mozilla` and then `firefox/` (BEXT-04).
const EXTENSION_HOSTS_DEBOUNCE: Duration = Duration::from_secs(2);
/// How often the files are checked when no watcher can be started.
const POLL_EVERY: Duration = Duration::from_secs(2);

/// Start every watcher and the tasks that answer their notifications; the
/// service aborts the handles on shutdown.
#[must_use]
pub fn spawn(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    std::iter::once(tokio::spawn(run(ctx.clone())))
        .chain(api::default_browser::spawn_tasks(ctx))
        .chain(api::clipboard::spawn_tasks(ctx))
        .collect()
}

/// Watch the current environment until it changes, then start over.
async fn run(ctx: ServiceContext) {
    let mut environments = ctx.environment_changes();
    loop {
        let environment = environments.borrow_and_update().clone();
        let watching = async {
            match environment {
                Some(environment) => watch(&ctx, environment).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            changed = environments.changed() => {
                if changed.is_err() {
                    return;
                }
            }
            () = watching => return,
        }
    }
}

/// Watch `environment` for good, re-arming as needed. Without a watcher
/// (no inotify, or out of watches) the files are checked every
/// [`POLL_EVERY`] instead, so a change is still seen.
async fn watch(ctx: &ServiceContext, environment: Arc<Environment>) {
    let mut polling = false;
    loop {
        // Scanning here also warms the inventory before the first link.
        let plan = Plan::new(&environment, &browser_dirs(ctx, &environment).await);
        let (sender, mut events) = mpsc::unbounded_channel();
        let Some(_watcher) = start(&plan, sender) else {
            if !polling {
                tracing::warn!(every = ?POLL_EVERY, "cannot watch files; checking them every few seconds instead");
                polling = true;
            }
            catch_up(ctx, &environment).await;
            poll(ctx, &environment, &plan).await;
            continue;
        };
        catch_up(ctx, &environment).await;
        let mut pending: BTreeMap<Kind, Instant> = BTreeMap::new();
        loop {
            let next = pending.values().min().copied();
            let sleep = async {
                match next {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                path = events.recv() => {
                    let Some(path) = path else { return };
                    for kind in plan.classify(&path) {
                        pending.entry(kind).or_insert_with(|| Instant::now() + debounce(kind));
                    }
                }
                () = sleep => {
                    let now = Instant::now();
                    let due: Vec<Kind> = pending
                        .iter()
                        .filter(|(_, deadline)| **deadline <= now)
                        .map(|(kind, _)| *kind)
                        .collect();
                    pending.retain(|_, deadline| *deadline > now);
                    if handle(ctx, &environment, &due).await {
                        break;
                    }
                }
            }
        }
    }
}

/// Check `plan`'s files every [`POLL_EVERY`] and act on what changed;
/// returns when the watchers must be rebuilt.
async fn poll(ctx: &ServiceContext, environment: &Arc<Environment>, plan: &Plan) {
    let paths = plan.polled();
    let mut seen = stamps(paths.clone()).await;
    loop {
        tokio::time::sleep(POLL_EVERY).await;
        let now = stamps(paths.clone()).await;
        let kinds: BTreeSet<Kind> = now
            .iter()
            .filter(|(path, stamp)| seen.get(*path) != Some(*stamp))
            .flat_map(|(path, _)| plan.classify(path))
            .collect();
        seen = now;
        if !kinds.is_empty()
            && handle(ctx, environment, &kinds.into_iter().collect::<Vec<_>>()).await
        {
            return;
        }
    }
}

/// What a path looks like now, for [`poll`]: where it leads and the
/// modification times of the link and of what it leads to. A Nix profile
/// swaps a symlink to files that all have the same modification time (1),
/// so the target's own time alone would miss the change.
type Stamp = (Option<PathBuf>, Option<SystemTime>, Option<SystemTime>);

/// The [`Stamp`] of each of `paths`.
async fn stamps(paths: Vec<PathBuf>) -> BTreeMap<PathBuf, Stamp> {
    blocking(move || {
        paths
            .into_iter()
            .map(|path| {
                let modified = |meta: std::io::Result<std::fs::Metadata>| {
                    meta.and_then(|meta| meta.modified()).ok()
                };
                let stamp = (
                    std::fs::canonicalize(&path).ok(),
                    modified(std::fs::symlink_metadata(&path)),
                    modified(std::fs::metadata(&path)),
                );
                (path, stamp)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

const fn debounce(kind: Kind) -> Duration {
    match kind {
        Kind::Config => CONFIG_DEBOUNCE,
        Kind::Registration | Kind::Inventory | Kind::Rearm => INVENTORY_DEBOUNCE,
        Kind::ExtensionHosts => EXTENSION_HOSTS_DEBOUNCE,
    }
}

/// Act on the kinds whose quiet time passed; true when the watchers must
/// be rebuilt.
async fn handle(ctx: &ServiceContext, environment: &Arc<Environment>, due: &[Kind]) -> bool {
    let mut rearm = due.contains(&Kind::Rearm);
    for kind in due {
        match kind {
            Kind::Config => {
                if let Err(error) = api::config::reload(ctx).await {
                    tracing::warn!(%error, "cannot reload the configuration");
                }
            }
            Kind::Registration => api::default_browser::check_takeover(ctx).await,
            Kind::Inventory => {
                let before = api::inventory::revision(ctx).await;
                match api::inventory::scan(ctx).await {
                    // New browsers bring new profile files to watch.
                    Ok(scan) => rearm |= scan.revision != before,
                    Err(error) => tracing::warn!(%error, "cannot rescan the apps"),
                }
            }
            Kind::ExtensionHosts => api::extension::refresh(ctx, environment.clone()).await,
            Kind::Rearm => {}
        }
    }
    rearm
}

/// Read what may have changed while no watcher was armed (at start and
/// between two watchers). The first time this also loads the configuration,
/// remembers the default-browser registration later changes are compared
/// with, and writes the extension's host manifests (BEXT-04).
async fn catch_up(ctx: &ServiceContext, environment: &Arc<Environment>) {
    if let Err(error) = api::config::reload(ctx).await {
        tracing::warn!(%error, "cannot read the configuration");
    }
    api::default_browser::check_takeover(ctx).await;
    api::extension::refresh(ctx, environment.clone()).await;
}

/// The config directories of the installed web browsers, whose profile
/// files are watched (DISC-02).
async fn browser_dirs(ctx: &ServiceContext, environment: &Environment) -> Vec<PathBuf> {
    let Ok(scan) = api::inventory::current(ctx).await else {
        return Vec::new();
    };
    scan.inventory
        .web_handlers()
        .into_iter()
        .flat_map(|app| wye_desktop::family::config_dirs(app.id(), &environment.xdg))
        .collect()
}

/// A notify watcher over `plan`'s directories that forwards changed paths
/// to `sender`, or `None` when notify is unavailable.
fn start(plan: &Plan, sender: mpsc::UnboundedSender<PathBuf>) -> Option<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            // Reading a file or listing a directory is not a change, and
            // Wye's own reads would otherwise trigger themselves.
            Ok(event) if matches!(event.kind, notify::EventKind::Access(_)) => {}
            Ok(event) => {
                for path in event.paths {
                    // The receiver is gone once the watcher is being rebuilt.
                    let _ = sender.send(path);
                }
            }
            Err(error) => tracing::debug!(%error, "file watcher error"),
        }
    })
    .inspect_err(|error| tracing::warn!(%error, "cannot watch files"))
    .ok()?;
    for dir in &plan.dirs {
        if let Err(error) = watcher.watch(dir, RecursiveMode::NonRecursive) {
            tracing::debug!(dir = %dir.display(), %error, "cannot watch a directory");
        }
    }
    Some(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A retargeted symlink is a change even when both targets carry the
    /// same modification time, as Nix store files do.
    #[tokio::test]
    async fn a_retargeted_symlink_changes_its_stamp() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        std::fs::write(&first, "same").expect("written");
        std::fs::copy(&first, &second).expect("copied");
        let times = std::fs::File::options()
            .write(true)
            .open(&first)
            .and_then(|file| file.metadata())
            .and_then(|meta| meta.modified())
            .expect("mtime");
        std::fs::File::options()
            .write(true)
            .open(&second)
            .and_then(|file| file.set_modified(times))
            .expect("same mtime");
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink(&first, &link).expect("linked");
        let before = stamps(vec![link.clone()]).await;
        std::fs::remove_file(&link).expect("unlinked");
        std::os::unix::fs::symlink(&second, &link).expect("relinked");
        let after = stamps(vec![link.clone()]).await;
        assert_ne!(before.get(&link), after.get(&link));
    }
}
