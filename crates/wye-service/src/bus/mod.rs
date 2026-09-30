//! The D-Bus objects, and the helpers topics use to announce changes.
//!
//! Every interface impl is written once, here, and only delegates to
//! [`crate::api`]; topics never edit these files. To tell clients that
//! something changed, a topic calls [`property_changed`] or one of the
//! signal helpers.

mod application;
mod kwin;
mod wye1;

pub use application::Application;
pub use kwin::KWin1;
pub use wye1::Wye1;

use wye_api::names::OBJECT_PATH;
use zbus::object_server::SignalEmitter;

use crate::context::ServiceContext;

/// Export every interface at [`OBJECT_PATH`] and remember `connection` in
/// `ctx`.
///
/// Call this before requesting the bus name: the object has to answer the
/// very first call a `Type=dbus` unit or a bus activation sends.
///
/// # Errors
///
/// A [`zbus::Error`] when an interface is already served at the path.
pub async fn serve(connection: &zbus::Connection, ctx: &ServiceContext) -> zbus::Result<()> {
    let server = connection.object_server();
    server.at(OBJECT_PATH, Wye1::new(ctx.clone())).await?;
    server
        .at(OBJECT_PATH, Application::new(ctx.clone()))
        .await?;
    server.at(OBJECT_PATH, KWin1::new(ctx.clone())).await?;
    if !ctx.attach(connection) {
        tracing::warn!("the service context was already attached to a connection");
    }
    Ok(())
}

/// A `dev.soldunov.wye1` property whose value changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    Tray,
    Status,
    ConfigRevision,
    HistoryRevision,
    InventoryRevision,
}

/// Emit `PropertiesChanged` for `property`, with its current value.
///
/// # Errors
///
/// A [`zbus::Error`] when the service is not served or the signal cannot be
/// sent.
pub async fn property_changed(ctx: &ServiceContext, property: Property) -> zbus::Result<()> {
    let connection = connection(ctx)?;
    let interface = connection
        .object_server()
        .interface::<_, Wye1>(OBJECT_PATH)
        .await?;
    let emitter = interface.signal_emitter();
    let wye1 = interface.get().await;
    match property {
        Property::Tray => wye1.tray_changed(emitter).await,
        Property::Status => wye1.status_changed(emitter).await,
        Property::ConfigRevision => wye1.config_revision_changed(emitter).await,
        Property::HistoryRevision => wye1.history_revision_changed(emitter).await,
        Property::InventoryRevision => wye1.inventory_revision_changed(emitter).await,
    }
}

/// Emit `MenuRequested`.
///
/// # Errors
///
/// A [`zbus::Error`] when the service is not served or the signal cannot be
/// sent.
pub async fn menu_requested(ctx: &ServiceContext) -> zbus::Result<()> {
    Wye1::menu_requested(&emitter(ctx)?).await
}

/// Emit `ScriptFileChanged(scope)` (SCR-08).
///
/// # Errors
///
/// A [`zbus::Error`] when the service is not served or the signal cannot be
/// sent.
pub async fn script_file_changed(ctx: &ServiceContext, scope: &str) -> zbus::Result<()> {
    Wye1::script_file_changed(&emitter(ctx)?, scope).await
}

fn connection(ctx: &ServiceContext) -> zbus::Result<&zbus::Connection> {
    ctx.connection()
        .ok_or_else(|| zbus::Error::Failure("the service is not on a bus yet".to_owned()))
}

fn emitter(ctx: &ServiceContext) -> zbus::Result<SignalEmitter<'static>> {
    SignalEmitter::new(connection(ctx)?, OBJECT_PATH)
}
