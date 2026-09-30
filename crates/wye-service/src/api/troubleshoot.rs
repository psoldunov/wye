//! `GetTroubleshooting`: the About window's troubleshooting text
//! (DLG-ABT-02).

use wye_api::Error;

use super::Result;
use crate::context::ServiceContext;

/// `dev.soldunov.wye1.GetTroubleshooting`: plain text, one fact per line.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_troubleshooting(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetTroubleshooting"))
}
