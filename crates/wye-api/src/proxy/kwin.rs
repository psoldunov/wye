//! `dev.soldunov.wye.KWin1` on the service (internal).

use zbus::proxy;

use crate::Error;

/// Called back by the one-shot `KWin` script with the pointer and the active
/// window (PICK-02, source-app step 4).
#[proxy(
    interface = "dev.soldunov.wye.KWin1",
    default_service = "dev.soldunov.wye",
    default_path = "/dev/soldunov/wye"
)]
pub trait KWin1 {
    /// One answer to the query started with `nonce`.
    #[allow(
        clippy::too_many_arguments,
        reason = "the signature is fixed by what the KWin script can send"
    )]
    fn report(
        &self,
        nonce: &str,
        x: i32,
        y: i32,
        output: &str,
        pid: u32,
        desktop_file: &str,
        resource_class: &str,
    ) -> Result<(), Error>;
}
