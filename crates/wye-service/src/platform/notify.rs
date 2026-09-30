//! Desktop notifications (PIPE-02, LAUNCH-07, DEF-03, SCR-22, DLG-EXP-04).
//!
//! Planned: a small `org.freedesktop.Notifications` client with actions and
//! the `desktop-entry=dev.soldunov.wye` hint. Until then nothing is shown.

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{Notification, NotificationAction, Notifier, PlatformError};

/// How many button presses a slow listener may fall behind.
const ACTION_BUFFER: usize = 16;

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
            "notifications are not implemented yet".to_owned(),
        ))
    }

    fn actions(&self) -> broadcast::Receiver<NotificationAction> {
        self.actions.subscribe()
    }
}
