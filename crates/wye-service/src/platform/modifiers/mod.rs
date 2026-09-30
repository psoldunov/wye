//! Held-modifier detection (BRW-03, RUL-27, KEY-06).
//!
//! Planned: a transient Wayland layer surface with exclusive keyboard focus
//! (`wayland.rs`), an X11 pointer query (`x11.rs`), setting
//! `advanced.held-keys = "auto" | "off"`. Until then modifiers are unknown.

use async_trait::async_trait;
use wye_api::context::Modifier;

use super::ModifierSource;

/// Modifiers are never known.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoModifiers;

#[async_trait]
impl ModifierSource for NoModifiers {
    async fn held(&self) -> Option<Vec<Modifier>> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}
