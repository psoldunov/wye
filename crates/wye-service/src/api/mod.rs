//! What each D-Bus member does, one module per topic.
//!
//! The interface impls in [`crate::bus`] only unpack arguments and call a
//! function here, so each topic lives in its own file. Each module also owns
//! a `State` type that [`crate::context::ServiceContext`] holds one of.
//!
//! A member that is not implemented yet answers
//! `dev.soldunov.wye.Error.NotImplemented`.

pub mod clipboard;
pub mod config;
pub mod default_browser;
pub mod expansion;
pub mod history;
pub mod inventory;
pub mod link;
pub mod picker;
pub mod picker_fallback;
pub mod rules;
pub mod scripts;
pub mod shortcuts;
pub mod state;
pub mod tray;
pub mod troubleshoot;
pub mod windows;

use std::collections::HashMap;

use zbus::message::Header;
use zbus::zvariant::OwnedValue;

/// An `a{sv}` argument as the service receives it.
pub type Dict = HashMap<String, OwnedValue>;

/// What every topic function returns.
pub type Result<T> = std::result::Result<T, wye_api::Error>;

/// Who called: the unique bus name of the sender.
///
/// Used where the caller matters: the caller's PID for source-app detection
/// (IN-01), the tray host's connection for `RegisterTray`, the `KWin`
/// script's replies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caller {
    /// Unique name such as `:1.42`; `None` on a peer-to-peer connection.
    pub sender: Option<String>,
}

impl Caller {
    /// The sender of the message `header` belongs to.
    #[must_use]
    pub fn from_header(header: &Header<'_>) -> Self {
        Self {
            sender: header.sender().map(ToString::to_string),
        }
    }
}
