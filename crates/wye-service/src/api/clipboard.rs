//! The clipboard: `OpenClipboard`, `ClipboardHasUrl`, and the copy-time
//! rewrites (IN-02 to IN-04, TRAY-10, EXT-02 to EXT-15).

use wye_api::Error;

use super::{Caller, Result};
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

/// `dev.soldunov.wye1.OpenClipboard` (IN-02 to IN-04).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn open_clipboard(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _alternative: bool,
) -> Result<()> {
    Err(Error::not_implemented("OpenClipboard"))
}

/// `dev.soldunov.wye1.ClipboardHasUrl` (TRAY-10).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn clipboard_has_url(_ctx: &ServiceContext) -> Result<bool> {
    Err(Error::not_implemented("ClipboardHasUrl"))
}
