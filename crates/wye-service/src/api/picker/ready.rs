//! Keeping `wye-ui` started while a route can end on the picker (decision
//! 2, PICK-25): at start, when a configuration change first makes the
//! picker reachable, and again after the UI host exits, so the next picker
//! does not wait for Qt to load (and for the `ShowPicker` deadline).

use std::time::Duration;

use futures_lite::StreamExt as _;
use tokio::time::Instant;
use wye_api::names::UI_BUS_NAME;
use wye_core::Config;
use zbus::fdo::DBusProxy;

use super::activation::can_end_on_picker;
use super::host;
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
    tokio::spawn(async move { activate(&ctx).await });
}

async fn activate_if_needed(ctx: &ServiceContext) {
    let needed = link::with_snapshot(ctx, |snapshot| {
        can_end_on_picker(snapshot.pipeline.config())
    })
    .await;
    match needed {
        Ok(true) => activate(ctx).await,
        Ok(false) => {}
        Err(error) => tracing::warn!(%error, "cannot read the configuration"),
    }
}

async fn activate(ctx: &ServiceContext) {
    if let Err(error) = host::activate(ctx).await {
        tracing::info!(%error, "cannot start the UI host ahead of time");
    }
}
