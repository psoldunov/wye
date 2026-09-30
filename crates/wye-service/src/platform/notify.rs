//! Desktop notifications (PIPE-02, LAUNCH-07, DEF-03, SCR-22, DLG-EXP-04).
//!
//! [`DesktopNotifier`] is a small `org.freedesktop.Notifications` client:
//! the `desktop-entry` hint names Wye's desktop entry, and buttons come back
//! as [`NotificationAction`]s from the `ActionInvoked` signal. notify-rust
//! is not used: it needs a newer Rust and blocks a thread per action wait.

use std::collections::HashMap;

use async_trait::async_trait;
use futures_lite::StreamExt as _;
use tokio::sync::{OnceCell, broadcast};
use zbus::zvariant::Value;

use super::{Notification, NotificationAction, Notifier, PlatformError};

/// How many button presses a slow listener may fall behind.
const ACTION_BUFFER: usize = 16;

/// Application name the notification server shows.
const APP_NAME: &str = "Wye";

/// Icon used when a notification names none.
const APP_ICON: &str = "dev.soldunov.wye";

/// Desktop entry the server groups Wye's notifications under.
const DESKTOP_ENTRY: &str = "dev.soldunov.wye";

/// Let the server decide how long a notification stays.
const DEFAULT_TIMEOUT: i32 = -1;

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(
        clippy::too_many_arguments,
        reason = "the signature is fixed by the notification specification"
    )]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

/// Notifications over the session bus.
#[derive(Debug)]
pub struct DesktopNotifier {
    connection: zbus::Connection,
    proxy: OnceCell<NotificationsProxy<'static>>,
    actions: broadcast::Sender<NotificationAction>,
}

impl DesktopNotifier {
    /// A notifier on `connection`; nothing is sent until the first
    /// notification.
    #[must_use]
    pub fn new(connection: zbus::Connection) -> Self {
        Self {
            connection,
            proxy: OnceCell::new(),
            actions: broadcast::Sender::new(ACTION_BUFFER),
        }
    }

    /// The proxy, listening for button presses from the first use on, so no
    /// press of a notification sent through it is missed.
    async fn proxy(&self) -> Result<&NotificationsProxy<'static>, PlatformError> {
        self.proxy
            .get_or_try_init(|| async {
                let proxy = NotificationsProxy::new(&self.connection)
                    .await
                    .map_err(|error| failed(&error))?;
                let mut presses = proxy
                    .receive_action_invoked()
                    .await
                    .map_err(|error| failed(&error))?;
                let actions = self.actions.clone();
                tokio::spawn(async move {
                    while let Some(signal) = presses.next().await {
                        match signal.args() {
                            Ok(args) => {
                                // Nobody listening means nobody cares.
                                let _ = actions.send(NotificationAction {
                                    id: args.id,
                                    action: args.action_key,
                                });
                            }
                            Err(error) => tracing::warn!(%error, "unreadable ActionInvoked"),
                        }
                    }
                });
                Ok(proxy)
            })
            .await
    }
}

#[async_trait]
impl Notifier for DesktopNotifier {
    async fn notify(&self, notification: &Notification) -> Result<u32, PlatformError> {
        let proxy = self.proxy().await?;
        let actions: Vec<&str> = notification
            .actions
            .iter()
            .flat_map(|(key, label)| [key.as_str(), label.as_str()])
            .collect();
        let hints = HashMap::from([("desktop-entry", Value::from(DESKTOP_ENTRY))]);
        proxy
            .notify(
                APP_NAME,
                notification.replaces.unwrap_or(0),
                notification.icon.as_deref().unwrap_or(APP_ICON),
                &notification.summary,
                &notification.body,
                &actions,
                hints,
                DEFAULT_TIMEOUT,
            )
            .await
            .map_err(|error| failed(&error))
    }

    fn actions(&self) -> broadcast::Receiver<NotificationAction> {
        self.actions.subscribe()
    }
}

fn failed(error: &zbus::Error) -> PlatformError {
    PlatformError::Failed(format!("notification server: {error}"))
}

/// Notifications are unavailable.
#[derive(Debug)]
pub struct NoNotifier {
    actions: broadcast::Sender<NotificationAction>,
}

impl NoNotifier {
    /// Refuses every notification.
    #[must_use]
    pub fn new() -> Self {
        Self {
            actions: broadcast::Sender::new(ACTION_BUFFER),
        }
    }
}

impl Default for NoNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Notifier for NoNotifier {
    async fn notify(&self, _notification: &Notification) -> Result<u32, PlatformError> {
        Err(PlatformError::Unavailable(
            "no notification server".to_owned(),
        ))
    }

    fn actions(&self) -> broadcast::Receiver<NotificationAction> {
        self.actions.subscribe()
    }
}
