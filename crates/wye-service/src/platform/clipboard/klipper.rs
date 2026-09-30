//! Plasma's clipboard manager, Klipper, over D-Bus (risk 9): the fallback
//! when the session offers no data control. Klipper does not keep entries a
//! password manager marked secret, so they never reach Wye (EXT-12).

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use futures_lite::StreamExt as _;
use tokio::sync::broadcast;

use super::{ClipboardCapabilities, ClipboardProvider, PlatformError, single_line_text};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "klipper";
/// Changes the watchers may fall behind by.
const BUFFER: usize = 16;

/// Klipper's D-Bus interface.
#[zbus::proxy(
    interface = "org.kde.klipper.klipper",
    default_service = "org.kde.klipper",
    default_path = "/klipper"
)]
trait Klipper {
    /// The current clipboard text.
    #[zbus(name = "getClipboardContents")]
    fn get_clipboard_contents(&self) -> zbus::Result<String>;

    /// Replace the clipboard text.
    #[zbus(name = "setClipboardContents")]
    fn set_clipboard_contents(&self, contents: &str) -> zbus::Result<()>;

    /// The history changed: something new was copied.
    #[zbus(signal, name = "clipboardHistoryUpdated")]
    fn clipboard_history_updated(&self) -> zbus::Result<()>;
}

/// The clipboard through Klipper.
#[derive(Debug)]
pub struct KlipperClipboard {
    proxy: KlipperProxy<'static>,
    changes: broadcast::Sender<String>,
}

impl KlipperClipboard {
    /// Talk to Klipper on `session`; fails when it does not answer.
    ///
    /// # Errors
    ///
    /// `Unavailable` when Klipper is not running.
    pub async fn start(session: &zbus::Connection) -> Result<Self, PlatformError> {
        let proxy = KlipperProxy::new(session)
            .await
            .map_err(|error| PlatformError::Unavailable(format!("no Klipper: {error}")))?;
        let current = proxy
            .get_clipboard_contents()
            .await
            .map_err(|error| PlatformError::Unavailable(format!("no Klipper: {error}")))?;
        let changes = broadcast::Sender::new(BUFFER);
        tokio::spawn(watch(
            proxy.clone(),
            changes.clone(),
            Arc::new(Mutex::new(current)),
        ));
        Ok(Self { proxy, changes })
    }
}

/// Announce every new clipboard text Klipper reports.
async fn watch(
    proxy: KlipperProxy<'static>,
    changes: broadcast::Sender<String>,
    last: Arc<Mutex<String>>,
) {
    let mut history = match proxy.receive_clipboard_history_updated().await {
        Ok(history) => history,
        Err(error) => {
            tracing::warn!(%error, "cannot watch Klipper");
            return;
        }
    };
    while history.next().await.is_some() {
        let Ok(text) = proxy.get_clipboard_contents().await else {
            continue;
        };
        let is_new = {
            let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
            let differs = *last != text;
            last.clone_from(&text);
            differs
        };
        if let Some(line) = is_new.then(|| single_line_text(&text)).flatten() {
            // Nobody watching is fine: rewrites may be off.
            let _ = changes.send(line);
        }
    }
}

#[async_trait]
impl ClipboardProvider for KlipperClipboard {
    async fn read(&self) -> Result<Option<String>, PlatformError> {
        let text = self
            .proxy
            .get_clipboard_contents()
            .await
            .map_err(|error| PlatformError::Failed(format!("Klipper: {error}")))?;
        Ok(single_line_text(&text))
    }

    async fn write(&self, text: &str) -> Result<(), PlatformError> {
        self.proxy
            .set_clipboard_contents(text)
            .await
            .map_err(|error| PlatformError::Failed(format!("Klipper: {error}")))
    }

    fn watch(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.changes.subscribe())
    }

    fn capabilities(&self) -> ClipboardCapabilities {
        ClipboardCapabilities {
            read: Some(MECHANISM),
            watch: Some(MECHANISM),
            write: Some(MECHANISM),
        }
    }
}
