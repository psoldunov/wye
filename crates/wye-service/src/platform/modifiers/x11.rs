//! Held modifiers on X11: the modifier mask `QueryPointer` returns (KEY-06).

use async_trait::async_trait;
use wye_api::context::Modifier;

use super::ModifierSource;
use crate::platform::PlatformError;
use crate::platform::x11::{self, MECHANISM};

/// Held modifiers from the X server.
#[derive(Debug, Clone, Copy, Default)]
pub struct X11Modifiers;

impl X11Modifiers {
    /// The start-up self-check: the X server answers a pointer query.
    ///
    /// # Errors
    ///
    /// When it does not.
    pub async fn check() -> Result<Self, PlatformError> {
        x11::held_modifiers().await.map(|_| Self)
    }
}

#[async_trait]
impl ModifierSource for X11Modifiers {
    async fn held(&self) -> Option<Vec<Modifier>> {
        x11::held_modifiers()
            .await
            .inspect_err(|error| tracing::info!(%error, "held modifiers unknown"))
            .ok()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}
