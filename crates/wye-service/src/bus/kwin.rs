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
        clippy::needless_pass_by_value,
        reason = "the signature is fixed by what the KWin script can send, and zbus \
                  hands the header over by value"
    )]
    fn report(
        &self,
        #[zbus(header)] header: Header<'_>,
        nonce: String,
        x: i32,
        y: i32,
        output: String,
        pid: i32,
        desktop_file: String,
        resource_class: String,
    ) -> Result<(), Error> {
        let report = KWinReport {
            nonce,
            pointer: Placement { output, x, y },
            // KWin's `Window::pid` is an `int`, and `callDBus` sends every
            // JavaScript integer as `i`; none (0 or -1) becomes 0.
            pid: u32::try_from(pid).unwrap_or(0),
            desktop_file,
            resource_class,
        };
        kwin::report(&self.ctx, &Caller::from_header(&header), report)
    }
}
