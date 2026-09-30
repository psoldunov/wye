//! What follows a configuration change: telling clients, one notification
//! when the file breaks (risk 16), and the autostart entry (GEN-01).

use std::path::PathBuf;

use wye_desktop::autostart;

use super::cache::Current;
use crate::bus::{self, Property};
use crate::context::{ServiceContext, blocking};
use crate::platform::Notification;

/// Tell clients that `after` replaced `before`, and act on what changed.
/// `saved` is true when the change came from `UpdateConfig` rather than
/// from the file.
pub(crate) async fn announce(
    ctx: &ServiceContext,
    before: Option<&Current>,
    after: &Current,
    saved: bool,
) {
    changed(ctx, Property::ConfigRevision);
    changed(ctx, Property::Status);
    // The tray's radio group and icon follow the configuration.
    changed(ctx, Property::Tray);
    let newly_broken = after.error.is_some() && before.is_none_or(|before| before.error.is_none());
    if newly_broken {
        notify_broken(ctx, after).await;
    }
    let login_changed = before.is_some_and(|before| {
        before.config.general.launch_at_login != after.config.general.launch_at_login
    });
    // A managed login start clears Wye's own entry once, at the first load.
    let first_managed = before.is_none() && ctx.login_managed().is_some();
    if saved || login_changed || first_managed {
        sync_autostart(ctx, after).await;
    }
    // Decision 2: the picker just became reachable; start the UI host now.
    crate::api::picker::ready::config_changed(
        ctx,
        before.map(|before| &before.config),
        &after.config,
    );
}

/// Emit `PropertiesChanged` from a task of its own; a service not on a bus
/// yet has nobody to tell.
///
/// Spawned rather than awaited: reading the new value of `Status` can load
/// the configuration, which announces changes of its own.
pub(crate) fn changed(ctx: &ServiceContext, property: Property) {
    if ctx.connection().is_none() {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = bus::property_changed(&ctx, property).await {
            tracing::debug!(%error, ?property, "cannot announce a property change");
        }
    });
}

/// Risk 16: the file is broken; the last good configuration stays.
async fn notify_broken(ctx: &ServiceContext, current: &Current) {
    let notification = Notification {
        summary: "Wye cannot read its configuration".to_owned(),
        body: format!(
            "{} has an error, so Wye keeps using the last working settings: {}",
            current.environment.config.display(),
            current.error.as_deref().unwrap_or_default()
        ),
        ..Notification::default()
    };
    if let Err(error) = ctx.platform().notifier.notify(&notification).await {
        tracing::warn!(%error, "cannot show the configuration notification");
    }
}

/// GEN-01: the autostart entry follows `general.launch-at-login`. A managed
/// entry (a symlink) is left alone and logged; so is everything while login
/// start is managed outside Wye (`WYE_LOGIN_MANAGED`), when only an entry
/// Wye wrote itself is removed, so the session does not start Wye twice.
pub(crate) async fn sync_autostart(ctx: &ServiceContext, current: &Current) {
    if ctx.login_managed().is_some() {
        remove_own_entry(current).await;
        return;
    }
    let xdg = current.environment.xdg.clone();
    let enabled = current.config.general.launch_at_login;
    let explicit = ctx.wye_executable_override();
    let result = blocking(move || {
        if enabled {
            let wye = wye_executable(explicit, &xdg).ok_or_else(|| {
                "cannot find the wye executable for the autostart entry".to_owned()
            })?;
            autostart::enable(&xdg, &wye).map_err(|error| error.to_string())
        } else {
            autostart::disable(&xdg)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!(%error, "cannot update the autostart entry"),
        Err(error) => tracing::warn!(%error, "cannot update the autostart entry"),
    }
}

/// GEN-01: login start is managed elsewhere; an autostart entry Wye wrote
/// would start it a second time (or against the manager's "off").
async fn remove_own_entry(current: &Current) {
    let xdg = current.environment.xdg.clone();
    match blocking(move || autostart::remove_own(&xdg)).await {
        Ok(Ok(true)) => {
            tracing::info!("login start is managed elsewhere; removed Wye's own autostart entry");
        }
        Ok(Ok(false)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "cannot remove Wye's own autostart entry"),
        Err(error) => tracing::warn!(%error, "cannot remove Wye's own autostart entry"),
    }
}

/// The `wye` the autostart entry runs: the one set explicitly, else `wye` on
/// `PATH` (a profile link such as `/etc/profiles/per-user/<user>/bin/wye`
/// survives updates, unlike a Nix store path), else this executable.
fn wye_executable(explicit: Option<PathBuf>, xdg: &wye_desktop::XdgDirs) -> Option<PathBuf> {
    explicit
        .or_else(|| xdg.find_program("wye"))
        .or_else(|| std::env::current_exe().ok())
}
