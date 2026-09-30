//! `KWin` helper: pointer position and the active window on Plasma (PICK-02,
//! source-app step 4).
//!
//! Planned: load a one-shot `KWin` script through `org.kde.kwin.Scripting`
//! that calls back `dev.soldunov.wye.KWin1.Report` with a nonce; 150 ms
//! timeout. Until then the pointer is unknown and reports are refused.

use async_trait::async_trait;
use wye_api::Error;
use wye_api::picker::Placement;

use super::PointerSource;
use crate::api::Caller;
use crate::context::ServiceContext;

/// One answer from the `KWin` script (`KWin1.Report`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KWinReport {
    /// The query this answers.
    pub nonce: String,
    /// Pointer position and output.
    pub pointer: Placement,
    /// Active window's process ID; 0 when none.
    pub pid: u32,
    /// Active window's desktop file name; empty when none.
    pub desktop_file: String,
    /// Active window's resource class; empty when none.
    pub resource_class: String,
}

/// Handle `KWin1.Report`.
///
/// # Errors
///
/// `NotImplemented` until the `KWin` helper exists.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits it; the real handler waits on the pending query"
)]
pub(crate) async fn report(
    _ctx: &ServiceContext,
    _caller: &Caller,
    _report: KWinReport,
) -> Result<(), Error> {
    Err(Error::not_implemented("KWin1.Report"))
}

/// The pointer is never known.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPointer;

#[async_trait]
impl PointerSource for NoPointer {
    async fn pointer(&self) -> Option<Placement> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}
