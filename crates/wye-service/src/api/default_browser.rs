//! Default-browser registration: `MakeDefault`, `StopBeingDefault`,
//! `KeepCurrentDefault` (DEF-02, DEF-03, DEF-05, ONB-10, ONB-11).

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

/// `dev.soldunov.wye1.MakeDefault` (DEF-02).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn make_default(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("MakeDefault"))
}

/// `dev.soldunov.wye1.StopBeingDefault` (DEF-05).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn stop_being_default(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("StopBeingDefault"))
}

/// `dev.soldunov.wye1.KeepCurrentDefault` (ONB-11).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn keep_current_default(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("KeepCurrentDefault"))
}
