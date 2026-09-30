//! X11 session probes: pointer (`QueryPointer`), modifiers (its mask) and the
//! active window (`_NET_ACTIVE_WINDOW`, `_NET_WM_PID`,
//! `_KDE_NET_WM_DESKTOP_FILE`, `_GTK_APPLICATION_ID`).
//!
//! Planned with x11rb. Until then [`X11Probe`] knows nothing.

use async_trait::async_trait;
use wye_api::picker::Placement;

use super::{FocusSource, FocusedApp, PointerSource};

/// X11 pointer and focus; not implemented yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct X11Probe;

#[async_trait]
impl PointerSource for X11Probe {
    async fn pointer(&self) -> Option<Placement> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

#[async_trait]
impl FocusSource for X11Probe {
    async fn focused(&self) -> Option<FocusedApp> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}
