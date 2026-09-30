//! The `StatusNotifierItem` tray (TRAY-01 to TRAY-18 on hosts without the
//! Plasma applet), through ksni.
//!
//! The item is a menu (`ItemIsMenu`, TRAY-07): a primary click opens it. The
//! icon is the `Tray` model's (TRAY-02, GEN-02), with a warning overlay while
//! Wye is not the default browser (ONB-11). Opening the menu reports
//! `AboutToShow` so the service can refresh the clipboard item (TRAY-10).

pub mod menu;

use std::time::Duration;

use async_trait::async_trait;
use ksni::{Category, Status, ToolTip, TrayMethods as _};
use tokio::sync::broadcast;
use wye_api::tray::{TrayMenu, TrayOverlay};

use super::{PlatformError, StatusNotifier, TrayEvent};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "status-notifier-item";

/// Decision 8: on KDE the Plasma applet usually registers within moments
/// of login; the item waits this long for it.
pub const KDE_GRACE: Duration = Duration::from_secs(5);

/// The item's ID, stable across sessions.
const ITEM_ID: &str = "dev.soldunov.wye";

/// The item's title.
const TITLE: &str = "Wye";

/// Emblem over the icon while Wye is not the default browser (ONB-11).
pub const WARNING_OVERLAY: &str = "emblem-warning";

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

    fn grace(&self) -> Duration {
        Duration::ZERO
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

/// The item, served by ksni on its own session-bus connection.
pub struct KsniNotifier {
    handle: tokio::sync::Mutex<Option<ksni::Handle<WyeTray>>>,
    events: broadcast::Sender<TrayEvent>,
    grace: Duration,
}

impl KsniNotifier {
    /// A tray that waits `grace` at start for a tray host.
    #[must_use]
    pub fn new(grace: Duration) -> Self {
        Self {
            handle: tokio::sync::Mutex::new(None),
            events: broadcast::channel(EVENTS).0,
            grace,
        }
    }

    /// The tray for this session: [`KDE_GRACE`] when `XDG_CURRENT_DESKTOP`
    /// names KDE, none otherwise.
    #[must_use]
    pub fn for_session() -> Self {
        let desktops = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        Self::new(grace_for(&desktops))
    }
}

/// How long to wait for a tray host on the desktops `XDG_CURRENT_DESKTOP`
/// names.
#[must_use]
pub fn grace_for(current_desktops: &str) -> Duration {
    let kde = current_desktops
        .split(':')
        .any(|desktop| desktop.eq_ignore_ascii_case("KDE"));
    if kde { KDE_GRACE } else { Duration::ZERO }
}

impl std::fmt::Debug for KsniNotifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KsniNotifier")
            .field("grace", &self.grace)
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

    fn grace(&self) -> Duration {
        self.grace
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// What ksni serves: the latest `Tray` model.
struct WyeTray {
    menu: TrayMenu,
    events: broadcast::Sender<TrayEvent>,
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
            title: TITLE.to_owned(),
            description: if self.warning() {
                "Wye is not the default browser".to_owned()
            } else {
                String::new()
            },
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
    use super::*;

    #[test]
    fn only_kde_waits_for_its_applet() {
        assert_eq!(grace_for("KDE"), KDE_GRACE);
        assert_eq!(grace_for("ubuntu:KDE"), KDE_GRACE);
        assert_eq!(grace_for("GNOME"), Duration::ZERO);
        assert_eq!(grace_for(""), Duration::ZERO);
    }

    #[tokio::test]
    async fn without_a_tray_nothing_shows() {
        let tray = NoStatusNotifier::new();
        assert!(tray.show(&TrayMenu::default()).await.is_err());
        assert_eq!(tray.mechanism(), None);
    }
}
