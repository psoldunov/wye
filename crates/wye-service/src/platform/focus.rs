//! The focused window's app (source-app step 4).
//!
//! Planned: the `KWin` helper on Plasma ([`super::kwin`]), `_NET_ACTIVE_WINDOW`
//! on X11 ([`super::x11`]). Until then the focused app is unknown.

use async_trait::async_trait;

use super::{FocusSource, FocusedApp};

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
