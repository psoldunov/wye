//! Clipboard access (IN-02 to IN-04, TRAY-10, EXT-12 to EXT-15, risk 9).
//!
//! In order of preference, probed at start ([`detect`]):
//!
//! - `wayland`: `ext-data-control-v1` / `zwlr-data-control-v1`: read, watch
//!   and write.
//! - `x11`: XFIXES selection events: read, watch and write.
//! - `klipper`: Plasma's clipboard manager over D-Bus, when the session
//!   offers neither: read, watch (its history signal) and write.
//!
//! Every provider hands on one line of plain text only, and never content
//! marked secret by a password manager or offered with an image (EXT-12).

pub mod klipper;
pub mod wayland;
pub mod x11;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{ClipboardCapabilities, ClipboardProvider, PlatformError};

/// Plain-text formats, most preferred first.
pub(crate) const TEXT_MIMES: [&str; 5] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "TEXT",
    "STRING",
];

/// Password managers mark secrets with this format (EXT-12).
pub(crate) const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";

/// Whether an offer with these formats must be left alone: a
/// password-manager entry (whatever the hint says, it came from a password
/// manager) or rich content with an image (EXT-12).
pub(crate) fn is_blocked<S: AsRef<str>>(formats: &[S]) -> bool {
    formats.iter().any(|format| {
        let format = format.as_ref();
        format == PASSWORD_HINT || format.starts_with("image/")
    })
}

/// The text when it is one line (the only clipboard content Wye uses).
pub(crate) fn single_line_text(text: &str) -> Option<String> {
    wye_core::clipboard::single_line(text).map(str::to_owned)
}

/// The best clipboard this session offers (risk 9): data control on
/// Wayland, XFIXES on X11, else Klipper on Plasma, else none.
pub async fn detect(session: &zbus::Connection) -> Arc<dyn ClipboardProvider> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        match wayland::DataControlClipboard::start().await {
            Ok(clipboard) => return Arc::new(clipboard),
            Err(error) => tracing::info!(%error, "no Wayland data control"),
        }
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_some() {
        match x11::XfixesClipboard::start().await {
            Ok(clipboard) => return Arc::new(clipboard),
            Err(error) => tracing::info!(%error, "no X11 clipboard"),
        }
    }
    match klipper::KlipperClipboard::start(session).await {
        Ok(clipboard) => return Arc::new(clipboard),
        Err(error) => tracing::info!(%error, "no Klipper"),
    }
    Arc::new(NoClipboard)
}

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
    PlatformError::Unavailable("this session does not let apps read the clipboard".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext12_secrets_and_images_are_left_alone() {
        assert!(is_blocked(&["text/plain", PASSWORD_HINT]));
        assert!(is_blocked(&["image/png", "text/plain"]));
        assert!(!is_blocked(&["text/plain", "text/html", "text/x-moz-url"]));
    }

    #[test]
    fn only_one_line_is_used() {
        assert_eq!(
            single_line_text("  https://example.com/ \n"),
            Some("https://example.com/".to_owned())
        );
        assert_eq!(single_line_text("a\nb"), None);
    }
}
