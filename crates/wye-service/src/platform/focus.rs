//! The focused window's app (source-app step 4) and the pointer (PICK-02):
//! which of the session's helpers answers them.
//!
//! On Plasma (X11 or Wayland) the `KWin` helper ([`super::kwin`]) answers
//! both with one script; on another X11 session the X server does
//! ([`super::x11`]). Elsewhere both are unknown: the picker is centred and
//! rules on source apps do not match links from sandboxed apps.

use std::sync::Arc;

use async_trait::async_trait;

use super::kwin::{KWinHelper, NoPointer, Reports};
use super::x11::X11Probe;
use super::{FocusSource, FocusedApp, PointerSource};

/// The focused app is never known.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFocus;

#[async_trait]
impl FocusSource for NoFocus {
    async fn focused(&self) -> Option<FocusedApp> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

/// The pointer and focus sources this session supports. `KWin` scripts
/// answer to `connection` through `reports`.
pub async fn detect(
    connection: &zbus::Connection,
    reports: &Reports,
) -> (Arc<dyn PointerSource>, Arc<dyn FocusSource>) {
    if let Some(helper) = KWinHelper::detect(connection, reports.clone()).await {
        let helper = Arc::new(helper);
        return (Arc::clone(&helper) as _, helper as _);
    }
    if is_x11_session() {
        match X11Probe::check().await {
            Ok(probe) => return (Arc::new(probe), Arc::new(probe)),
            Err(error) => tracing::info!(%error, "no X11 pointer or focus"),
        }
    }
    (Arc::new(NoPointer), Arc::new(NoFocus))
}

/// An X11 session, not `Xwayland` beside a Wayland one: on Wayland the X
/// server only sees its own clients' windows and the pointer over them.
fn is_x11_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_some()
}
