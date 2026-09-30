//! Rules: `TestLink`, `ExportRules`, `ImportRules` (IN-08, DLG-TST, RUL-02).

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

/// `dev.soldunov.wye1.TestLink` (IN-08): JSON [`wye_api::trace::LinkTrace`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn test_link(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _url: &str,
    _context: &Dict,
) -> Result<String> {
    Err(Error::not_implemented("TestLink"))
}

/// `dev.soldunov.wye1.ExportRules` (RUL-02): TOML with rules and scripts.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn export_rules(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("ExportRules"))
}

/// `dev.soldunov.wye1.ImportRules` (RUL-02): appends; returns how many.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn import_rules(_ctx: &ServiceContext, _text: &str) -> Result<u32> {
    Err(Error::not_implemented("ImportRules"))
}
