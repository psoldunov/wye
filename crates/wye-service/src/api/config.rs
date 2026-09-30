//! The configuration: `GetConfig`, `UpdateConfig`, `SetPrimary`,
//! `GetDefaults` and the `ConfigRevision` property (SET-06, TRAY-11, KEY-04).
//!
//! Writes keep the existing contract: re-serialise only when
//! `Loaded::is_lossless`, atomically. A broken file keeps the last good
//! configuration and reports it in `Status`.

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

/// The `ConfigRevision` property.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn revision(_ctx: &ServiceContext) -> u64 {
    0
}

/// `dev.soldunov.wye1.GetConfig`: the configuration as JSON and its
/// revision.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_config(_ctx: &ServiceContext) -> Result<(String, u64)> {
    Err(Error::not_implemented("GetConfig"))
}

/// `dev.soldunov.wye1.UpdateConfig` (SET-06): RFC 7386 merge patch; returns
/// the new revision.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn update_config(
    _ctx: &ServiceContext,
    _merge_patch: &str,
    _base_revision: u64,
) -> Result<u64> {
    Err(Error::not_implemented("UpdateConfig"))
}

/// `dev.soldunov.wye1.SetPrimary` (TRAY-11).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn set_primary(_ctx: &ServiceContext, _target: &str) -> Result<()> {
    Err(Error::not_implemented("SetPrimary"))
}

/// `dev.soldunov.wye1.GetDefaults` (KEY-04).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_defaults(_ctx: &ServiceContext, _section: &str) -> Result<String> {
    Err(Error::not_implemented("GetDefaults"))
}
