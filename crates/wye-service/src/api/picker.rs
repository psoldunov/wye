//! The picker broker: `PreviewPicker`, `PickerChose`, `PickerCancelled`,
//! `PickerAction` (PICK-01 to PICK-33, PIPE-13, PKS-06, PKS-07, IN-06).
//!
//! The service sends `PickerHost1.ShowPicker` to the UI host and waits for
//! one of these calls back. A new link supersedes a pending request
//! (PICK-27).

use wye_api::Error;

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

/// `dev.soldunov.wye1.PreviewPicker` (IN-06, PKS-06).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn preview_picker(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("PreviewPicker"))
}

/// `dev.soldunov.wye1.PickerChose` (PIPE-13).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn picker_chose(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _request_id: &str,
    _target: &str,
    _options: &Dict,
) -> Result<()> {
    Err(Error::not_implemented("PickerChose"))
}

/// `dev.soldunov.wye1.PickerCancelled` (PICK-23).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn picker_cancelled(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _request_id: &str,
) -> Result<()> {
    Err(Error::not_implemented("PickerCancelled"))
}

/// `dev.soldunov.wye1.PickerAction` ([`wye_api::actions::PickerAction`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn picker_action(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _request_id: &str,
    _action: &str,
) -> Result<()> {
    Err(Error::not_implemented("PickerAction"))
}
