//! History: `GetHistory`, `ClearHistory`, `DeleteHistoryEntry`,
//! `ReopenHistoryEntry` and the `HistoryRevision` property (DLG-HIS-01 to
//! DLG-HIS-04, PIPE-16, ADV-09, TRAY-15).
//!
//! Stored as JSON in `$XDG_STATE_HOME/wye/history.json`, 100 entries
//! (decision 10).

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

/// The `HistoryRevision` property.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn revision(_ctx: &ServiceContext) -> u64 {
    0
}

/// `dev.soldunov.wye1.GetHistory`: JSON [`wye_api::history::History`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_history(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetHistory"))
}

/// `dev.soldunov.wye1.ClearHistory` (DLG-HIS-01, ADV-09).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn clear_history(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("ClearHistory"))
}

/// `dev.soldunov.wye1.DeleteHistoryEntry` (DLG-HIS-03).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn delete_history_entry(_ctx: &ServiceContext, _id: u64) -> Result<()> {
    Err(Error::not_implemented("DeleteHistoryEntry"))
}

/// `dev.soldunov.wye1.ReopenHistoryEntry` ([`wye_api::actions::Reopen`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn reopen_history_entry(_ctx: &ServiceContext, _id: u64, _how: &str) -> Result<()> {
    Err(Error::not_implemented("ReopenHistoryEntry"))
}
