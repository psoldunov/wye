//! The configuration: `GetConfig`, `UpdateConfig`, `SetPrimary`,
//! `GetDefaults` and the `ConfigRevision` property (SET-06, TRAY-11, KEY-04).
//!
//! The service keeps the configuration in memory and reloads it when the
//! file changes ([`reload`], called by [`crate::watch`]). Writes keep the
//! existing contract: re-serialise only when `Loaded::is_lossless`,
//! atomically. A broken file keeps the last good configuration and reports
//! it in `Status` (risk 16).

mod cache;
mod defaults;
pub(crate) mod effects;
mod update;

use std::sync::{Arc, PoisonError, RwLock};

use serde_json::Value;
use wye_api::{Error, json};
use wye_core::Target;

pub(crate) use self::cache::Current;
use super::Result;
use super::link::Snapshot;
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    current: RwLock<Option<Arc<Current>>>,
    /// Serialises loads and writes, so a reload never interleaves with a
    /// save.
    writing: tokio::sync::Mutex<()>,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }

    fn get(&self) -> Option<Arc<Current>> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, current: Arc<Current>) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Some(current);
    }
}

/// The configuration in use, read on first use and again whenever the
/// environment changes.
///
/// # Errors
///
/// `Failed` when there is no home directory.
pub(crate) async fn current(ctx: &ServiceContext) -> Result<Arc<Current>> {
    let environment = ctx.environment()?;
    if let Some(current) = ctx.config().get()
        && Arc::ptr_eq(&current.environment, &environment)
    {
        return Ok(current);
    }
    reload(ctx).await
}

/// Read the file again and keep what it says (SET-06 reload, risk 16).
/// Clients hear about it only when something changed.
///
/// # Errors
///
/// `Failed` when there is no home directory.
pub(crate) async fn reload(ctx: &ServiceContext) -> Result<Arc<Current>> {
    let _writing = ctx.config().writing.lock().await;
    let environment = ctx.environment()?;
    let before = ctx.config().get();
    let last_good = before.clone();
    let loaded = blocking(move || cache::load(environment, last_good.as_deref())).await?;
    if let Some(before) = &before
        && before.same_as(&loaded)
    {
        return Ok(before.clone());
    }
    let revision = before.as_ref().map_or(1, |before| before.revision + 1);
    let current = Arc::new(Current { revision, ..loaded });
    ctx.config().set(current.clone());
    effects::announce(ctx, before.as_deref(), &current, false).await;
    Ok(current)
}

/// The `ConfigRevision` property; 0 before the file could be read at all.
pub async fn revision(ctx: &ServiceContext) -> u64 {
    current(ctx).await.map_or(0, |current| current.revision)
}

/// `dev.soldunov.wye1.GetConfig`: the configuration as JSON, in the file's
/// shape (kebab-case keys), and its revision.
pub async fn get_config(ctx: &ServiceContext) -> Result<(String, u64)> {
    let current = current(ctx).await?;
    Ok((json::encode(&current.config)?, current.revision))
}

/// `dev.soldunov.wye1.UpdateConfig` (SET-06): RFC 7386 merge patch; returns
/// the new revision.
pub async fn update_config(
    ctx: &ServiceContext,
    merge_patch: &str,
    base_revision: u64,
) -> Result<u64> {
    let patch: Value = serde_json::from_str(merge_patch)
        .map_err(|error| Error::invalid_args(format!("merge patch: {error}")))?;
    apply_patch(ctx, &patch, base_revision).await
}

/// Apply `patch` and save (SET-06). A patch that changes nothing writes
/// nothing and keeps the revision.
///
/// # Errors
///
/// `Conflict`, `ReadOnly`, `NotLossless`, `InvalidArgs` (see
/// `docs/dbus-api.md`), `Failed` when the file cannot be written.
pub(crate) async fn apply_patch(
    ctx: &ServiceContext,
    patch: &Value,
    base_revision: u64,
) -> Result<u64> {
    let before = current(ctx).await?;
    let writing = ctx.config().writing.lock().await;
    // A reload may have landed while waiting for the lock.
    let before = ctx.config().get().unwrap_or(before);
    update::writable(&before, base_revision)?;
    let checked = update::check(&before, patch)?;
    if checked.loaded.config == before.config {
        drop(writing);
        // GEN-01: confirming the default ("launch at login" on) still
        // creates the entry.
        effects::sync_autostart(ctx, &before).await;
        return Ok(before.revision);
    }
    let file = before.environment.config.clone();
    let text = checked.text;
    blocking(move || wye_desktop::atomic::write(&file, text.as_bytes(), Some(&file)))
        .await?
        .map_err(|error| {
            Error::failed(format!(
                "cannot save {}: {error}",
                before.environment.config.display()
            ))
        })?;
    let after = Arc::new(cache::from_loaded(
        before.environment.clone(),
        checked.loaded,
        before.writable,
        before.revision + 1,
    ));
    ctx.config().set(after.clone());
    drop(writing);
    effects::announce(ctx, Some(&before), &after, true).await;
    Ok(after.revision)
}

/// Whether `current` may be saved at all (`ReadOnly`, `NotLossless`), for
/// callers that prepare files before patching.
///
/// # Errors
///
/// See [`apply_patch`].
pub(crate) fn check_writable(current: &Current) -> Result<()> {
    update::writable(current, 0)
}

/// `dev.soldunov.wye1.SetPrimary` (TRAY-11): `browsers.primary`.
pub async fn set_primary(ctx: &ServiceContext, target: &str) -> Result<()> {
    let target: Target = json::decode("target", target)?;
    let patch = serde_json::json!({
        "browsers": { "primary": wye_core::merge_patch::target_patch(&target) }
    });
    apply_patch(ctx, &patch, 0).await.map(|_| ())
}

/// `dev.soldunov.wye1.GetDefaults` (KEY-04).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one reads nothing"
)]
pub async fn get_defaults(_ctx: &ServiceContext, section: &str) -> Result<String> {
    json::encode(&defaults::section(section)?)
}

/// Everything one link is routed with: the cached configuration and
/// inventory, and the browser Wye replaced as the default. This replaces
/// reading the files for every link (`Snapshot::load`).
///
/// # Errors
///
/// `Failed` when there is no home directory.
pub(crate) async fn snapshot(ctx: &ServiceContext) -> Result<Snapshot> {
    let config = current(ctx).await?;
    let inventory = super::inventory::current(ctx).await?;
    let state = super::state::load(ctx).await.unwrap_or_default();
    Ok(Snapshot {
        pipeline: (*config.pipeline).clone(),
        inventory: (*inventory.inventory).clone(),
        previous_default: state.previous_default_browser.clone(),
    })
}
