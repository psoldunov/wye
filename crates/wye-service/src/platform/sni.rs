//! The `StatusNotifierItem` tray (TRAY-01 to TRAY-18), through ksni: Wye's
//! tray on every desktop with a tray host, KDE Plasma included.
//!
//! The item is a menu (`ItemIsMenu`, TRAY-07): a primary click opens it. A
//! middle click (`SecondaryActivate`) opens Settings (TRAY-19), the one thing
//! worth a shortcut past the menu. The icon is the `Tray` model's (TRAY-02,
//! GEN-02), with a warning overlay and `NeedsAttention` while Wye is not the
//! default browser (TRAY-18, ONB-11); the tooltip names the primary browser.
//! Opening the menu reports `AboutToShow` so the service can refresh the
//! clipboard item (TRAY-10).

pub mod menu;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ksni::{Category, Status, ToolTip, TrayMethods as _};
use tokio::sync::broadcast;
use wye_api::tray::{APP_ICON, TrayItemKind, TrayMenu, TrayOverlay};
use wye_core::tray::ids;

use super::{PlatformError, StatusNotifier, TrayEvent};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "status-notifier-item";

/// The item's ID, stable across sessions: the app ID, so a tray host keeps
/// the user's per-item settings (shown, hidden) between logins.
pub const ITEM_ID: &str = "dev.soldunov.wye";

/// The item's title, and the tooltip's.
pub const TITLE: &str = "Wye";

/// Emblem over the icon while Wye is not the default browser (ONB-11).
pub const WARNING_OVERLAY: &str = "emblem-warning";

/// Tooltip text while Wye is not the default browser (TRAY-18).
pub const NOT_DEFAULT: &str = "Wye is not the default browser";

/// Where the icon theme directory sits relative to the running binary in an
/// installed package (`<prefix>/bin/wye`, `<prefix>/share/icons`).
const ICONS_FROM_BIN: &str = "../share/icons";

/// Buffer of the events channel.
const EVENTS: usize = 16;

/// No tray: the item is never shown.
#[derive(Debug)]
pub struct NoStatusNotifier {
    events: broadcast::Sender<TrayEvent>,
}

impl NoStatusNotifier {
    /// A tray that shows nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: broadcast::channel(EVENTS).0,
        }
    }
}

impl Default for NoStatusNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatusNotifier for NoStatusNotifier {
    async fn show(&self, _menu: &TrayMenu) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable("no tray".to_owned()))
    }

    async fn hide(&self) {}

    fn events(&self) -> broadcast::Receiver<TrayEvent> {
        self.events.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

/// The item, served by ksni on its own session-bus connection.
pub struct KsniNotifier {
    handle: tokio::sync::Mutex<Option<ksni::Handle<WyeTray>>>,
    events: broadcast::Sender<TrayEvent>,
    icon_theme_path: String,
}

impl KsniNotifier {
    /// The tray item of this installation: icons are looked up in the
    /// package's own `share/icons` too, when there is one.
    #[must_use]
    pub fn new() -> Self {
        let icon_theme_path = std::env::current_exe()
            .ok()
            .and_then(|exe| installed_icons(&exe))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            handle: tokio::sync::Mutex::new(None),
            events: broadcast::channel(EVENTS).0,
            icon_theme_path,
        }
    }
}

impl Default for KsniNotifier {
    fn default() -> Self {
        Self::new()
    }
}

/// The icon theme directory of the package `exe` belongs to, when it holds
/// Wye's own tray icon: tray hosts then find the icon even when the package
/// is not on `XDG_DATA_DIRS` (`IconThemePath`).
#[must_use]
pub fn installed_icons(exe: &Path) -> Option<PathBuf> {
    let icons = exe.parent()?.join(ICONS_FROM_BIN).canonicalize().ok()?;
    let own = icons.join(format!("hicolor/symbolic/apps/{APP_ICON}.svg"));
    own.is_file().then_some(icons)
}

impl std::fmt::Debug for KsniNotifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KsniNotifier")
            .field("icon_theme_path", &self.icon_theme_path)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl StatusNotifier for KsniNotifier {
    async fn show(&self, menu: &TrayMenu) -> Result<(), PlatformError> {
        let mut handle = self.handle.lock().await;
        if let Some(running) = handle.as_ref().filter(|running| !running.is_closed()) {
            let next = menu.clone();
            if running.update(move |tray| tray.menu = next).await.is_some() {
                return Ok(());
            }
        }
        let tray = WyeTray {
            menu: menu.clone(),
            events: self.events.clone(),
            icon_theme_path: self.icon_theme_path.clone(),
        };
        // A tray host that starts later (or restarts) still finds the item.
        let spawned = tray
            .assume_sni_available(true)
            .spawn()
            .await
            .map_err(|error| {
                PlatformError::Failed(format!("cannot show the tray item: {error}"))
            })?;
        *handle = Some(spawned);
        Ok(())
    }

    async fn hide(&self) {
        let handle = self.handle.lock().await.take();
        if let Some(handle) = handle {
            handle.shutdown().await;
        }
    }

    fn events(&self) -> broadcast::Receiver<TrayEvent> {
        self.events.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// What ksni serves: the latest `Tray` model.
struct WyeTray {
    menu: TrayMenu,
    events: broadcast::Sender<TrayEvent>,
    icon_theme_path: String,
}

impl WyeTray {
    fn warning(&self) -> bool {
        self.menu.overlay == Some(TrayOverlay::Warning)
    }

    fn send(&self, event: TrayEvent) {
        // Nobody listening means the service is shutting down.
        let _ = self.events.send(event);
    }
}

/// The tooltip's second line: the default-browser warning (TRAY-18), else
/// the checked item of the "Primary Browser" radio group (TRAY-11).
#[must_use]
pub fn tool_tip_text(menu: &TrayMenu) -> String {
    if menu.overlay == Some(TrayOverlay::Warning) {
        return NOT_DEFAULT.to_owned();
    }
    menu.items
        .iter()
        .find(|item| item.kind == TrayItemKind::Radio && item.checked)
        .map(|primary| format!("Primary browser: {}", primary.label))
        .unwrap_or_default()
}

impl menu::Chooser for WyeTray {
    fn chosen(&mut self, id: &str) {
        self.send(TrayEvent::Activated(id.to_owned()));
    }
}

impl ksni::Tray for WyeTray {
    /// TRAY-07: a primary click opens the menu.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        ITEM_ID.to_owned()
    }

    fn title(&self) -> String {
        TITLE.to_owned()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn status(&self) -> Status {
        if self.warning() {
            Status::NeedsAttention
        } else {
            Status::Active
        }
    }

    /// A middle click opens Settings (TRAY-19); the window is TRAY-16's.
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayEvent::Activated(ids::SETTINGS.to_owned()));
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    fn icon_name(&self) -> String {
        self.menu.icon.name().to_owned()
    }

    fn attention_icon_name(&self) -> String {
        self.icon_name()
    }

    fn overlay_icon_name(&self) -> String {
        if self.warning() {
            WARNING_OVERLAY.to_owned()
        } else {
            String::new()
        }
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: self.icon_name(),
            title: TITLE.to_owned(),
            description: tool_tip_text(&self.menu),
            ..ToolTip::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        menu::items(&self.menu.items)
    }

    fn menu_about_to_show(&mut self) {
        self.send(TrayEvent::AboutToShow);
    }
}

#[cfg(test)]
mod tests {
    use wye_api::tray::TrayItem;

    use super::*;

    fn radio(label: &str, checked: bool) -> TrayItem {
        TrayItem {
            id: format!("primary:{label}"),
            kind: TrayItemKind::Radio,
            label: label.to_owned(),
            icon: None,
            shortcut: None,
            enabled: true,
            checked,
            children: Vec::new(),
        }
    }

    #[tokio::test]
    async fn without_a_tray_nothing_shows() {
        let tray = NoStatusNotifier::new();
        assert!(tray.show(&TrayMenu::default()).await.is_err());
        assert_eq!(tray.mechanism(), None);
    }

    #[test]
    fn the_tool_tip_names_the_primary_browser_tray_11() {
        let menu = TrayMenu {
            items: vec![radio("Picker", false), radio("Firefox", true)],
            ..TrayMenu::default()
        };
        assert_eq!(tool_tip_text(&menu), "Primary browser: Firefox");
        assert_eq!(tool_tip_text(&TrayMenu::default()), "");
    }

    #[test]
    fn the_tool_tip_warns_while_wye_is_not_the_default_tray_18() {
        let menu = TrayMenu {
            overlay: Some(TrayOverlay::Warning),
            items: vec![radio("Picker", true)],
            ..TrayMenu::default()
        };
        assert_eq!(tool_tip_text(&menu), NOT_DEFAULT);
    }

    #[test]
    fn the_package_icons_are_found_next_to_the_binary() {
        let root = tempfile::tempdir().expect("temp dir");
        let bin = root.path().join("bin");
        let apps = root.path().join("share/icons/hicolor/symbolic/apps");
        std::fs::create_dir_all(&bin).expect("bin");
        let exe = bin.join("wye");
        assert_eq!(installed_icons(&exe), None, "no share/icons");
        std::fs::create_dir_all(&apps).expect("icons");
        assert_eq!(installed_icons(&exe), None, "no Wye icon");
        std::fs::write(apps.join(format!("{APP_ICON}.svg")), "<svg/>").expect("icon");
        let icons = root
            .path()
            .join("share/icons")
            .canonicalize()
            .expect("canonical");
        assert_eq!(installed_icons(&exe), Some(icons));
    }
}
