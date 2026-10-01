//! Keep Qt ready when picker routes need it (PICK-25), but not while GNOME
//! Shell owns the picker. Redisplay a pending Shell request on Qt if Shell
//! leaves the bus before answering.

use std::time::Duration;

use futures_lite::StreamExt as _;
use tokio::time::Instant;
use wye_api::names::{GNOME_BUS_NAME, UI_BUS_NAME};
use wye_core::Config;
use zbus::fdo::DBusProxy;

use super::activation::can_end_on_picker;
use super::host;
use super::shell_left;
use crate::api::link;
use crate::context::ServiceContext;

/// Shortest time between two restarts of a UI host that exited, so one
/// that crashes at start is not restarted in a loop.
const RESTART_GAP: Duration = Duration::from_secs(30);

/// Start the UI host now when needed, then again whenever it leaves the
/// bus while it is still needed.
pub(super) async fn keep_ready(ctx: ServiceContext) {
    activate_if_needed(&ctx).await;
    let Some(connection) = ctx.connection().cloned() else {
        return;
    };
    let changes = async {
        DBusProxy::new(&connection)
            .await?
            .receive_name_owner_changed_with_args(&[(0, UI_BUS_NAME)])
            .await
    };
    let mut changes = match changes.await {
        Ok(changes) => changes,
        Err(error) => {
            tracing::info!(%error, "cannot follow the UI host; it starts with the next picker");
            return;
        }
    };
    let mut last: Option<Instant> = None;
    while let Some(change) = changes.next().await {
        if !change.args().is_ok_and(|args| args.new_owner().is_none()) {
            continue;
        }
        if let Some(last) = last {
            tokio::time::sleep(RESTART_GAP.saturating_sub(last.elapsed())).await;
        }
        last = Some(Instant::now());
        activate_if_needed(&ctx).await;
    }
}

/// When Shell loses its name, redisplay the still-pending request on Qt.
/// Do not start Qt on GNOME just to watch for this transition.
pub(super) async fn watch_shell(ctx: ServiceContext) {
    let Some(connection) = ctx.connection().cloned() else {
        return;
    };
    let changes = async {
        DBusProxy::new(&connection)
            .await?
            .receive_name_owner_changed_with_args(&[(0, GNOME_BUS_NAME)])
            .await
    };
    let mut changes = match changes.await {
        Ok(changes) => changes,
        Err(error) => {
            tracing::warn!(%error, "cannot watch the GNOME picker host");
            return;
        }
    };
    while let Some(change) = changes.next().await {
        if host::is_gnome(&ctx) && change.args().is_ok_and(|args| args.new_owner().is_none()) {
            shell_left(&ctx).await;
        }
    }
}

/// A configuration change: start the UI host when the picker just became
/// reachable.
pub(crate) fn config_changed(ctx: &ServiceContext, before: Option<&Config>, after: &Config) {
    if ctx.connection().is_none()
        || !can_end_on_picker(after)
        || before.is_some_and(can_end_on_picker)
    {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move { activate_if_qt_needed(&ctx).await });
}

async fn activate_if_needed(ctx: &ServiceContext) {
    let needed = link::with_snapshot(ctx, |snapshot| {
        can_end_on_picker(snapshot.pipeline.config())
    })
    .await;
    match needed {
        Ok(true) => activate_if_qt_needed(ctx).await,
        Ok(false) => {}
        Err(error) => tracing::warn!(%error, "cannot read the configuration"),
    }
}

async fn activate_if_qt_needed(ctx: &ServiceContext) {
    if !host::needs_qt_picker(ctx).await {
        return;
    }
    if let Err(error) = host::activate(ctx).await {
        tracing::info!(%error, "cannot start the UI host ahead of time");
    }
}
