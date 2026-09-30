//! Everything the service asks of the desktop session, behind traits.
//!
//! Each trait has a real implementation in its own module (filled in by the
//! unit that owns it), a no-op that reports "unavailable", and a fake in
//! [`fake`] for tests on a private bus without a desktop.
//!
//! Every `mechanism` method names what is in use (for example
//! `"wayland-layer-shell"` or `"portal"`) or returns `None` when the session
//! cannot do it; `Status.capabilities` reports these (KEY-06, DLG-ABT-02).

pub mod clipboard;
pub mod fake;
pub mod focus;
pub mod http;
pub mod kwin;
pub mod lock;
pub mod modifiers;
pub mod notify;
pub mod probe;
pub mod scope;
pub mod shortcuts;
pub mod sni;
pub mod types;
pub mod x11;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};
use wye_api::context::Modifier;
use wye_api::picker::Placement;
use wye_api::tray::TrayMenu;

pub use types::{
    BoundShortcut, ClipboardCapabilities, FocusedApp, HttpMethod, HttpRequest, HttpResponse,
    LaunchCommand, Notification, NotificationAction, PlatformError, TrayEvent,
};

/// Modifiers held right now (BRW-03, RUL-27, KEY-06).
#[async_trait]
pub trait ModifierSource: Send + Sync {
    /// The held modifiers, or `None` when they cannot be known.
    async fn held(&self) -> Option<Vec<Modifier>>;
    /// How modifiers are probed.
    fn mechanism(&self) -> Option<&'static str>;
}

/// The pointer, for placing the picker (PICK-02).
#[async_trait]
pub trait PointerSource: Send + Sync {
    /// Where the pointer is, or `None` when unknown.
    async fn pointer(&self) -> Option<Placement>;
    /// How the pointer is read.
    fn mechanism(&self) -> Option<&'static str>;
}

/// The focused window's app (source-app step 4).
#[async_trait]
pub trait FocusSource: Send + Sync {
    /// The focused app, or `None` when unknown.
    async fn focused(&self) -> Option<FocusedApp>;
    /// How focus is read.
    fn mechanism(&self) -> Option<&'static str>;
}

/// Screen-lock state (PKS-07).
pub trait LockMonitor: Send + Sync {
    /// Current state and changes: `true` while locked.
    fn locked(&self) -> watch::Receiver<bool>;
    /// Where the state comes from.
    fn mechanism(&self) -> Option<&'static str>;
}

/// Desktop notifications (PIPE-02, LAUNCH-07, DEF-03, SCR-22).
#[async_trait]
pub trait Notifier: Send + Sync {
    /// Show a notification; returns its ID.
    async fn notify(&self, notification: &Notification) -> Result<u32, PlatformError>;
    /// Buttons the user pressed.
    fn actions(&self) -> broadcast::Receiver<NotificationAction>;
}

/// Starting apps (LAUNCH-01 to LAUNCH-05).
#[async_trait]
pub trait Launcher: Send + Sync {
    /// Start the command; returns the process ID.
    async fn launch(&self, command: &LaunchCommand) -> Result<u32, PlatformError>;
}

/// Moving launched apps into their own systemd scope (LAUNCH-06).
#[async_trait]
pub trait ScopeManager: Send + Sync {
    /// Move `pid` into a transient scope for `command`'s app.
    async fn adopt(&self, pid: u32, command: &LaunchCommand) -> Result<(), PlatformError>;
}

/// The clipboard (IN-02 to IN-04, TRAY-10, EXT-12 to EXT-15).
#[async_trait]
pub trait ClipboardProvider: Send + Sync {
    /// The clipboard's single-line text, if any. A password manager's
    /// secret is never returned.
    async fn read(&self) -> Result<Option<String>, PlatformError>;
    /// Replace the clipboard's text (EXT-12).
    async fn write(&self, text: &str) -> Result<(), PlatformError>;
    /// New clipboard text as it is copied, when watching is possible.
    fn watch(&self) -> Option<broadcast::Receiver<String>>;
    /// What is possible in this session.
    fn capabilities(&self) -> ClipboardCapabilities;
}

/// Global shortcuts (KEY-40, KEY-41, ADV-05 to ADV-07).
#[async_trait]
pub trait ShortcutProvider: Send + Sync {
    /// The bindings as the mechanism reports them.
    async fn bindings(&self) -> Result<Vec<BoundShortcut>, PlatformError>;
    /// Ask for `trigger` on `action`; an empty trigger clears it.
    async fn bind(&self, action: &str, trigger: &str) -> Result<(), PlatformError>;
    /// Open the mechanism's own configuration dialog.
    async fn configure(&self) -> Result<(), PlatformError>;
    /// Action IDs as their shortcuts are pressed.
    fn activations(&self) -> broadcast::Receiver<String>;
    /// How shortcuts are registered.
    fn mechanism(&self) -> Option<&'static str>;
}

/// HTTP for short-link expansion and Songlink (PIPE-03, EXT-15). Blocking:
/// the pipeline calls it from `spawn_blocking`.
pub trait HttpClient: Send + Sync {
    /// Send one request.
    ///
    /// # Errors
    ///
    /// [`PlatformError::Timeout`] when no answer came in time,
    /// [`PlatformError::Failed`] for anything else.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, PlatformError>;
}

/// The `StatusNotifierItem` tray (TRAY-01 to TRAY-18 on hosts without the
/// Plasma applet): one item whose `DBusMenu` renders the `Tray` model.
#[async_trait]
pub trait StatusNotifier: Send + Sync {
    /// Show the item with `menu`, or update the one shown.
    async fn show(&self, menu: &TrayMenu) -> Result<(), PlatformError>;
    /// Remove the item; nothing happens when none is shown.
    async fn hide(&self);
    /// What the user did with the item.
    fn events(&self) -> broadcast::Receiver<TrayEvent>;
    /// How long the service waits after starting for a tray host to
    /// register before it shows the item (decision 8: 5 s on KDE).
    fn grace(&self) -> Duration;
    /// How the item is shown.
    fn mechanism(&self) -> Option<&'static str>;
}

/// One implementation of every platform trait.
#[derive(Clone)]
pub struct Platform {
    pub modifiers: Arc<dyn ModifierSource>,
    pub pointer: Arc<dyn PointerSource>,
    pub focus: Arc<dyn FocusSource>,
    pub lock: Arc<dyn LockMonitor>,
    pub notifier: Arc<dyn Notifier>,
    pub launcher: Arc<dyn Launcher>,
    pub scope: Arc<dyn ScopeManager>,
    pub clipboard: Arc<dyn ClipboardProvider>,
    pub shortcuts: Arc<dyn ShortcutProvider>,
    pub http: Arc<dyn HttpClient>,
    /// The `StatusNotifierItem` shown when no tray host registered (TRAY-01).
    pub sni: Arc<dyn StatusNotifier>,
    /// `KWin` queries waiting for their `KWin1.Report`; shared by the
    /// pointer and focus helper and the bus object that receives.
    pub kwin_reports: kwin::Reports,
}

impl Platform {
    /// Nothing available: every integration answers "unavailable" or
    /// "unknown". What the service falls back to when it cannot detect the
    /// session.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            modifiers: Arc::new(modifiers::NoModifiers),
            pointer: Arc::new(kwin::NoPointer),
            focus: Arc::new(focus::NoFocus),
            lock: Arc::new(lock::NoLockMonitor::new()),
            notifier: Arc::new(notify::NoNotifier::new()),
            launcher: Arc::new(scope::NoLauncher),
            scope: Arc::new(scope::NoScopes),
            clipboard: Arc::new(clipboard::NoClipboard),
            shortcuts: Arc::new(shortcuts::NoShortcuts::new()),
            http: Arc::new(http::NoHttp),
            sni: Arc::new(sni::NoStatusNotifier::new()),
            kwin_reports: kwin::Reports::new(),
        }
    }

    /// `self` with the session probes this session supports, each found by
    /// a start-up self-check (KEY-06): held modifiers (obeying
    /// `advanced.held-keys` in `config` when given), the pointer and the
    /// focused app. `KWin` scripts answer to `connection`, which must serve
    /// [`crate::bus::KWin1`].
    pub async fn with_session_probes(
        self,
        connection: &zbus::Connection,
        config: Option<&Path>,
    ) -> Self {
        let modifiers = match config {
            Some(config) => modifiers::detect_configured(config).await,
            None => modifiers::detect().await,
        };
        let (pointer, focus) = focus::detect(connection, &self.kwin_reports).await;
        tracing::info!(
            held_keys = modifiers.mechanism(),
            pointer = pointer.mechanism(),
            focus = focus.mechanism(),
            "session probes"
        );
        Self {
            modifiers,
            pointer,
            focus,
            ..self
        }
    }

    /// The integrations this session supports.
    ///
    /// For now the same as [`Platform::unavailable`]; each platform unit
    /// replaces its own entry with the real implementation as it lands.
    #[must_use]
    pub fn detect() -> Self {
        Self::unavailable()
    }
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Platform")
            .field("modifiers", &self.modifiers.mechanism())
            .field("pointer", &self.pointer.mechanism())
            .field("focus", &self.focus.mechanism())
            .field("lock", &self.lock.mechanism())
            .field("clipboard", &self.clipboard.capabilities())
            .field("shortcuts", &self.shortcuts.mechanism())
            .finish_non_exhaustive()
    }
}
