//! Keep the host of the next picker ready when picker routes need it
//! (PICK-25): Qt or GTK, whichever `advanced.frontend` puts first (ADV-12),
//! and none while GNOME Shell owns the picker. Redisplay a pending Shell
//! request elsewhere if Shell leaves the bus before answering.

use std::time::Duration;

use futures_lite::StreamExt as _;
use tokio::time::Instant;
use wye_api::names::{GNOME_BUS_NAME, GTK_BUS_NAME, UI_BUS_NAME};
use wye_core::Config;
use wye_core::config::Frontend;
use zbus::fdo::DBusProxy;

use super::activation::can_end_on_picker;
use super::frontend::Host;
use super::host;
use super::shell_left;
use crate::api::link;
use crate::context::ServiceContext;

/// Shortest time between two restarts of a UI host that exited, so one
/// that crashes at start is not restarted in a loop.
const RESTART_GAP: Duration = Duration::from_secs(30);

/// Start the UI host now when needed, then again whenever a host the
/// warm-up keeps ready leaves the bus while it is still needed.
pub(super) async fn keep_ready(ctx: ServiceContext) {
    if let Some(frontend) = needed_frontend(&ctx).await {
        activate_warm_host(&ctx, frontend).await;
    }
    let Some(connection) = ctx.connection().cloned() else {
        return;
    };
    let changes = async {
        let bus = DBusProxy::new(&connection).await?;
        let qt = bus
            .receive_name_owner_changed_with_args(&[(0, UI_BUS_NAME)])
            .await?;
        let gtk = bus
            .receive_name_owner_changed_with_args(&[(0, GTK_BUS_NAME)])
            .await?;
        Ok::<_, zbus::Error>(qt.or(gtk))
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
        let Ok(args) = change.args() else {
            continue;
        };
        if args.new_owner().is_some() {
            continue;
        }
        let Some(frontend) = needed_frontend(&ctx).await else {
            continue;
        };
        // Only a host the warm-up keeps ready restarts it, and only that
        // counts towards the gap between restarts.
        if !host::rewarms(&ctx, frontend, args.name().as_str()).await {
            continue;
        }
        if let Some(last) = last {
            tokio::time::sleep(RESTART_GAP.saturating_sub(last.elapsed())).await;
        }
        last = Some(Instant::now());
        activate_warm_host(&ctx, frontend).await;
    }
}

/// When the Shell loses its name, redisplay the request it shows on the
/// next host ([`shell_left`]). Do not start another host just to watch for
/// this transition.
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
        if change.args().is_ok_and(|args| args.new_owner().is_none()) {
            shell_left(&ctx).await;
        }
    }
}

/// A configuration change: start the host of the next picker when the
/// picker just became reachable, or when `advanced.frontend` changed
/// (ADV-12). A host already running stays, with any window it shows.
pub(crate) fn config_changed(ctx: &ServiceContext, before: Option<&Config>, after: &Config) {
    let frontend = after.advanced.frontend;
    let was_reachable = before.is_some_and(can_end_on_picker);
    let frontend_changed = before.is_some_and(|before| before.advanced.frontend != frontend);
    if ctx.connection().is_none()
        || !can_end_on_picker(after)
        || (was_reachable && !frontend_changed)
    {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move { activate_warm_host(&ctx, frontend).await });
}

/// `advanced.frontend` when a route can end on the picker; `None` when no
/// route can, so nothing needs to be ready.
async fn needed_frontend(ctx: &ServiceContext) -> Option<Frontend> {
    let needed = link::with_snapshot(ctx, |snapshot| {
        let config = snapshot.pipeline.config();
        can_end_on_picker(config).then_some(config.advanced.frontend)
    })
    .await;
    needed.unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read the configuration");
        None
    })
}

async fn activate_warm_host(ctx: &ServiceContext, frontend: Frontend) {
    let Some(host) = host::warm_host(ctx, frontend, None).await else {
        return;
    };
    if start(ctx, host).await && host::serves_picker(ctx, host).await {
        return;
    }
    // ADV-12: a host that cannot start, or a GTK host that serves windows
    // only, leaves the picker to the next frontend, which must be ready too.
    if let Some(next) = host::warm_host(ctx, frontend, Some(host)).await {
        start(ctx, next).await;
    }
}

/// Start `host`; whether it runs now.
async fn start(ctx: &ServiceContext, host: Host) -> bool {
    match host::activate(ctx, host).await {
        Ok(()) => true,
        Err(error) => {
            tracing::info!(%error, host = host.label(), "cannot start the UI host ahead of time");
            false
        }
    }
}
