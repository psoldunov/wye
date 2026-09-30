//! Global shortcuts and the tray-menu popup: `GetShortcuts`, `SetShortcut`,
//! `ConfigureShortcuts`, `ToggleMenu` (KEY-40, KEY-41, ADV-05 to ADV-07,
//! TRAY-08).
//!
//! `ToggleMenu` lives here because the toggle-menu shortcut is its main
//! caller; it shows the UI host's popup (decision 4).

use wye_api::Error;

use super::Result;
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

/// `dev.soldunov.wye1.GetShortcuts`: JSON [`wye_api::shortcuts::Shortcuts`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_shortcuts(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetShortcuts"))
}

/// `dev.soldunov.wye1.SetShortcut`; an empty binding clears it.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn set_shortcut(_ctx: &ServiceContext, _action: &str, _binding: &str) -> Result<()> {
    Err(Error::not_implemented("SetShortcut"))
}

/// `dev.soldunov.wye1.ConfigureShortcuts` (KEY-40 "Change…").
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn configure_shortcuts(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("ConfigureShortcuts"))
}

/// `dev.soldunov.wye1.ToggleMenu` (TRAY-08).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn toggle_menu(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("ToggleMenu"))
}
