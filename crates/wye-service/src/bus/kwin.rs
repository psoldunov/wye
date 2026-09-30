//! `dev.soldunov.wye.KWin1` (internal): the one-shot `KWin` script's
//! callback.

use wye_api::Error;
use wye_api::picker::Placement;
use zbus::message::Header;

use crate::api::Caller;
use crate::context::ServiceContext;
use crate::platform::kwin::{self, KWinReport};

/// Receives the `KWin` helper's answers.
#[derive(Debug, Clone)]
pub struct KWin1 {
    ctx: ServiceContext,
}

impl KWin1 {
    /// The interface over `ctx`.
    #[must_use]
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }
}

#[zbus::interface(name = "dev.soldunov.wye.KWin1")]
impl KWin1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the signature is fixed by what the KWin script can send"
    )]
    async fn report(
        &self,
        #[zbus(header)] header: Header<'_>,
        nonce: String,
        x: i32,
        y: i32,
        output: String,
        pid: u32,
        desktop_file: String,
        resource_class: String,
    ) -> Result<(), Error> {
        let report = KWinReport {
            nonce,
            pointer: Placement { output, x, y },
            pid,
            desktop_file,
            resource_class,
        };
        kwin::report(&self.ctx, &Caller::from_header(&header), report).await
    }
}
