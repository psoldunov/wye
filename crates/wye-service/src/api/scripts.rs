//! Transform scripts: `GetScript`, `SetScript`, `RunScript` and the
//! `ScriptFileChanged` signal (SCR-01 to SCR-09, SCR-20 to SCR-23).

use wye_api::Error;

use super::{Dict, Result};
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

/// `dev.soldunov.wye1.GetScript` ([`wye_api::actions::ScriptScope`]).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_script(_ctx: &ServiceContext, _scope: &str) -> Result<String> {
    Err(Error::not_implemented("GetScript"))
}

/// `dev.soldunov.wye1.SetScript`; `ScriptSyntax` when it does not compile
/// (SCR-07).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn set_script(_ctx: &ServiceContext, _scope: &str, _source: &str) -> Result<()> {
    Err(Error::not_implemented("SetScript"))
}

/// `dev.soldunov.wye1.RunScript` (SCR-04): JSON
/// [`wye_api::scripts::ScriptRun`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn run_script(
    _ctx: &ServiceContext,
    _source: &str,
    _url: &str,
    _context: &Dict,
) -> Result<String> {
    Err(Error::not_implemented("RunScript"))
}
