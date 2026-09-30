//! The tray: the `Tray` property, `RegisterTray`, and the
//! `StatusNotifierItem` shown when no tray host registered (TRAY-01 to
//! TRAY-18, decision 8).

use wye_api::Error;
use wye_api::json;
use wye_api::tray::TrayMenu;

use super::{Caller, Result};
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

/// The `Tray` property as JSON. An empty, hidden menu for now.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn tray_json(_ctx: &ServiceContext) -> Result<String> {
    json::encode(&TrayMenu::default())
}

/// `dev.soldunov.wye1.RegisterTray` ([`wye_api::actions::TrayHost`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn register_tray(_ctx: &ServiceContext, _caller: &Caller, _kind: &str) -> Result<()> {
    Err(Error::not_implemented("RegisterTray"))
}
