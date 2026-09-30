//! Windows and lifetime: `org.freedesktop.Application.Activate` and
//! `ActivateAction`, `ShowWindow`, `Quit` (TRAY-05, TRAY-16, TRAY-17,
//! SET-04).
//!
//! Windows are forwarded to the UI host (`dev.soldunov.wye.Windows1`).

use wye_api::Error;
use zbus::zvariant::OwnedValue;

use super::{Caller, Dict, Result};
use crate::context::ServiceContext;
use crate::platform::Platform;

/// State this topic keeps. Empty until the topic is implemented.
#[derive(Debug, Default)]
pub struct State;

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self
    }
}

/// `org.freedesktop.Application.Activate`: started without a link. First
/// run, else Settings when there is no tray icon (TRAY-05), else nothing.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn activate(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _platform_data: &Dict,
) -> Result<()> {
    Err(Error::not_implemented("Activate"))
}

/// `org.freedesktop.Application.ActivateAction`: a desktop action
/// ([`wye_api::actions::ApplicationAction`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn activate_action(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _action: &str,
    _parameter: &[OwnedValue],
    _platform_data: &Dict,
) -> Result<()> {
    Err(Error::not_implemented("ActivateAction"))
}

/// `dev.soldunov.wye1.ShowWindow` ([`wye_api::actions::Window`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn show_window(_ctx: &ServiceContext, _window: &str, _argument: &str) -> Result<()> {
    Err(Error::not_implemented("ShowWindow"))
}

/// `dev.soldunov.wye1.Quit` (TRAY-17): the service stops; the next link
/// starts it again through D-Bus activation.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn quit(ctx: &ServiceContext) -> Result<()> {
    ctx.request_shutdown();
    Ok(())
}
