//! Clipboard access (IN-02 to IN-04, TRAY-10, EXT-12 to EXT-15).
//!
//! Planned: `ext-data-control-v1` / `wlr-data-control` on Wayland
//! (`wayland.rs`), XFIXES selection events on X11 (`x11.rs`), Klipper's D-Bus
//! API as a read fallback on Plasma (`klipper.rs`). Until then the clipboard
//! is unavailable.

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{ClipboardCapabilities, ClipboardProvider, PlatformError};

/// The clipboard is unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClipboard;

#[async_trait]
impl ClipboardProvider for NoClipboard {
    async fn read(&self) -> Result<Option<String>, PlatformError> {
        Err(unavailable())
    }

    async fn write(&self, _text: &str) -> Result<(), PlatformError> {
        Err(unavailable())
    }

    fn watch(&self) -> Option<broadcast::Receiver<String>> {
        None
    }

    fn capabilities(&self) -> ClipboardCapabilities {
        ClipboardCapabilities::default()
    }
}

fn unavailable() -> PlatformError {
    PlatformError::Unavailable("clipboard access is not implemented yet".to_owned())
}
