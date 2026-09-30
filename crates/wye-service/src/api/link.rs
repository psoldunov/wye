//! Routing links: `OpenLink` and `org.freedesktop.Application.Open` (IN-01,
//! IN-05, IN-07, PIPE-01, PIPE-02, PIPE-12, PIPE-15, LAUNCH-03, LAUNCH-06).
//!
//! `Open` starts source-app detection at the caller's PID; `OpenLink` takes
//! the context keys in [`wye_api::context`]. The pipeline runs in
//! `spawn_blocking` because expansion and scripts may block.

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

/// `dev.soldunov.wye1.OpenLink`.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn open_link(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _url: &str,
    _context: &Dict,
) -> Result<()> {
    Err(Error::not_implemented("OpenLink"))
}

/// `org.freedesktop.Application.Open`: each URI enters the pipeline as a
/// handler link (IN-01).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn open_uris(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _uris: &[String],
    _platform_data: &Dict,
) -> Result<()> {
    Err(Error::not_implemented("Open"))
}
