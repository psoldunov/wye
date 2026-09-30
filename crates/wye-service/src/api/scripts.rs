//! Transform scripts: `GetScript`, `SetScript`, `RunScript` and the
//! `ScriptFileChanged` signal (SCR-01 to SCR-09, SCR-20 to SCR-23), and the
//! pipeline's [`Transformer`](wye_core::Transformer) (PIPE-05, PIPE-14).
//!
//! Scripts are files next to `config.toml` ([`files`]); the engine is
//! `wye-script`. The link path gets its transformer from [`transformer`].

mod failures;
mod files;
mod source;
mod trial;
mod watch;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use tokio::task::JoinHandle;
use wye_api::actions::ScriptScope;
use wye_api::{Error, json};
use wye_script::ScriptTransformer;

use self::files::ScriptFiles;
use self::source::FileScripts;
use super::{Dict, Result};
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// The pipeline's script hook: `transform.js` and `rules/<id>.js`, read
/// for every run.
pub(crate) type Scripts = ScriptTransformer<FileScripts>;

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    /// Hash of each script's text as the service last read or wrote it
    /// (`None`: no file), so the watcher only announces changes made
    /// elsewhere (SCR-08).
    seen: Mutex<HashMap<ScriptScope, Option<String>>>,
    /// Set once the file watcher runs.
    watching: OnceLock<()>,
    /// Serialises SCR-22's check-and-notify.
    reporting: tokio::sync::Mutex<()>,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }

    /// Record `text` as `scope`'s current text; true when it differs from
    /// what was recorded before (or nothing was).
    fn saw(&self, scope: &ScriptScope, text: Option<&str>) -> bool {
        let hash = text.map(|text| failures::script_hash("", text));
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.insert(scope.clone(), hash.clone()) != Some(hash)
    }
}

/// Background work of this topic: the script-file watcher (SCR-08). Empty
/// when it already runs; the first `GetScript` or `SetScript` also starts
/// it.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    start_watching(ctx).into_iter().collect()
}

fn start_watching(ctx: &ServiceContext) -> Option<JoinHandle<()>> {
    ctx.scripts().watching.set(()).ok()?;
    Some(tokio::spawn(watch::run(ctx.clone())))
}

/// The transformer for the link path, over the current environment's
/// script files; `None` without a home directory. Failures notify once per
/// script (SCR-22).
pub(crate) fn transformer(ctx: &ServiceContext) -> Option<Scripts> {
    let files = script_files(ctx).ok()?;
    Some(ScriptTransformer::new(FileScripts::new(ctx, files)))
}

/// [`transformer`] for `TestLink` (IN-08): nothing is notified.
pub(crate) fn trial_transformer(ctx: &ServiceContext) -> Option<Scripts> {
    let scripts = transformer(ctx)?;
    Some(ScriptTransformer::new(scripts.sources().clone().quiet()))
}

fn script_files(ctx: &ServiceContext) -> Result<ScriptFiles> {
    Ok(ScriptFiles::beside(&ctx.environment()?.config))
}

fn parse_scope(scope: &str) -> Result<ScriptScope> {
    scope
        .parse()
        .map_err(|error: wye_api::UnknownValue| Error::invalid_args(error.to_string()))
}

/// `dev.soldunov.wye1.GetScript`: the script's text; a missing or empty
/// file answers the template (SCR-03).
///
/// # Errors
///
/// `InvalidArgs` for an unknown scope, `Failed` when the file cannot be
/// read.
pub async fn get_script(ctx: &ServiceContext, scope: &str) -> Result<String> {
    let scope = parse_scope(scope)?;
    let files = script_files(ctx)?;
    start_watching(ctx);
    let reading = scope.clone();
    let text = blocking(move || files.read(&reading)).await??;
    ctx.scripts().saw(&scope, text.as_deref());
    Ok(text
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| wye_script::TEMPLATE.to_owned()))
}

/// `dev.soldunov.wye1.ScriptExists` (SCR-09): whether the scope's file
/// holds more than whitespace, so turning a transform on can open the
/// editor for a script that is still empty.
///
/// # Errors
///
/// `InvalidArgs` for an unknown scope, `Failed` when the file cannot be
/// read.
pub async fn script_exists(ctx: &ServiceContext, scope: &str) -> Result<bool> {
    let scope = parse_scope(scope)?;
    let files = script_files(ctx)?;
    let text = blocking(move || files.read(&scope)).await??;
    Ok(text.is_some_and(|text| !text.trim().is_empty()))
}

/// `dev.soldunov.wye1.SetScript`: save atomically; `ScriptSyntax` when it
/// does not compile (SCR-07).
///
/// # Errors
///
/// `InvalidArgs` for an unknown scope, `ScriptSyntax` (`line:column: text`),
/// `Failed` when the file cannot be written.
pub async fn set_script(ctx: &ServiceContext, scope: &str, source: &str) -> Result<()> {
    let scope = parse_scope(scope)?;
    let files = script_files(ctx)?;
    start_watching(ctx);
    let text = source.to_owned();
    let checked = text.clone();
    blocking(move || wye_script::check(&checked))
        .await?
        .map_err(|error| Error::ScriptSyntax(error.to_string()))?;
    // Recorded first, so the watcher sees its own write as known.
    ctx.scripts().saw(&scope, Some(&text));
    let writing = scope.clone();
    blocking(move || files.write(&writing, &text)).await??;
    tracing::info!(%scope, "script saved");
    Ok(())
}

/// `dev.soldunov.wye1.RunScript` (SCR-04): JSON
/// [`wye_api::scripts::ScriptRun`]. A script that fails is an answer with
/// `ok: false`, not a D-Bus error.
///
/// # Errors
///
/// `InvalidArgs` for a link that does not parse or a bad context value.
pub async fn run_script(
    _ctx: &ServiceContext,
    source: &str,
    url: &str,
    context: &Dict,
) -> Result<String> {
    let (url, input) = trial::input(url, context)?;
    let source = source.to_owned();
    let answer = blocking(move || {
        let run = wye_script::run(&source, &url, &input);
        for line in &run.logs {
            tracing::info!(script = "editor", "console.log: {line}");
        }
        trial::answer(&url, run)
    })
    .await?;
    json::encode(&answer)
}
