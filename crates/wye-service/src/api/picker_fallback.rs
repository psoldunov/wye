//! The stand-in when no picker can be shown: the UI host is unreachable.
//!
//! Moves today's `crates/wye/src/picker_fallback.rs` logic into the service:
//! the remembered previous default browser, else the first shown browser,
//! else the first discovered one, plus a notification.

use wye_api::Error;

use super::Result;
use crate::context::ServiceContext;

/// Open `url` without a picker.
#[allow(
    dead_code,
    clippy::unused_async,
    reason = "a stub: the picker broker will call and await it once the UI host can be unreachable"
)]
pub async fn open_without_picker(_ctx: &ServiceContext, _url: &str) -> Result<()> {
    Err(Error::not_implemented("the picker stand-in"))
}
