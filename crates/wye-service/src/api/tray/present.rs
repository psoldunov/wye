//! Showing the `StatusNotifierItem`: it runs while the menu is visible
//! (TRAY-04) and no external tray host registered, from the moment the
//! service starts; it follows every `Tray` change and carries out what is
//! chosen.

use std::sync::Arc;

use futures_lite::StreamExt as _;
use tokio::sync::broadcast::error::RecvError;
use wye_api::names::{INTERFACE, OBJECT_PATH};
use wye_api::tray::TrayMenu;
use zbus::fdo::{PropertiesChanged, PropertiesChangedStream, PropertiesProxy};
use zbus::proxy::CacheProperties;

use crate::context::ServiceContext;
use crate::platform::{StatusNotifier, TrayEvent};

/// The `Tray` property's name.
const TRAY_PROPERTY: &str = "Tray";

/// Run until the service stops.
pub(crate) async fn present(ctx: ServiceContext) {
    let sni = Arc::clone(&ctx.platform().sni);
    let mut events = sni.events();
    let mut hosts = ctx.tray().hosts.subscribe();
    let mut changes = tray_changes(&ctx).await;
    let mut shown: Option<TrayMenu> = None;
    loop {
        shown = sync(&ctx, sni.as_ref(), shown).await;
        ctx.tray().set_sni_shown(shown.is_some());
        tokio::select! {
            // A host registered or the last one left: the next sync hides or
            // shows the item.
            alive = hosts.changed() => {
                if alive.is_err() {
                    return;
                }
            }
            () = next_tray_change(&mut changes) => {}
            event = events.recv() => match event {
                Ok(TrayEvent::Activated(id)) => {
                    if let Err(error) = super::activate_tray_item(&ctx, &id).await {
                        tracing::warn!(%error, id, "the tray item failed");
                    }
                }
                // TRAY-10: the next sync reads the clipboard again.
                Ok(TrayEvent::AboutToShow) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            },
        }
    }
}

/// Show, update or hide the item for the menu as it is now; returns what
/// is shown.
async fn sync(
    ctx: &ServiceContext,
    sni: &dyn StatusNotifier,
    shown: Option<TrayMenu>,
) -> Option<TrayMenu> {
    let menu = match super::build(ctx).await {
        Ok(menu) => menu,
        Err(error) => {
            tracing::warn!(%error, "cannot build the tray menu");
            return shown;
        }
    };
    if !menu.visible || ctx.tray().hosts.any() {
        if shown.is_some() {
            sni.hide().await;
        }
        return None;
    }
    if shown.as_ref() == Some(&menu) {
        return shown;
    }
    match sni.show(&menu).await {
        Ok(()) => Some(menu),
        Err(error) => {
            tracing::info!(%error, "no tray item");
            None
        }
    }
}

/// `PropertiesChanged` of the service's own `dev.soldunov.wye1`: every topic
/// announces `Tray` changes there, so following it keeps the item in step
/// without each topic knowing about the tray. `None` when the service is not
/// on a bus.
async fn tray_changes(ctx: &ServiceContext) -> Option<PropertiesChangedStream> {
    let connection = ctx.connection()?;
    let own = connection.unique_name()?.to_owned();
    let proxy = PropertiesProxy::builder(connection)
        .destination(own)
        .ok()?
        .path(OBJECT_PATH)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .inspect_err(|error| tracing::warn!(%error, "cannot follow the Tray property"))
        .ok()?;
    proxy
        .receive_properties_changed()
        .await
        .inspect_err(|error| tracing::warn!(%error, "cannot follow the Tray property"))
        .ok()
}

/// Resolves on the next change of `Tray`; never without a stream.
async fn next_tray_change(changes: &mut Option<PropertiesChangedStream>) {
    let Some(stream) = changes.as_mut() else {
        return std::future::pending().await;
    };
    while let Some(signal) = stream.next().await {
        if is_tray_change(&signal) {
            return;
        }
    }
    *changes = None;
    std::future::pending::<()>().await;
}

fn is_tray_change(signal: &PropertiesChanged) -> bool {
    signal.args().is_ok_and(|args| {
        args.interface_name().as_str() == INTERFACE
            && (args.changed_properties().contains_key(TRAY_PROPERTY)
                || args.invalidated_properties().contains(&TRAY_PROPERTY))
    })
}
