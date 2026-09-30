//! Screen-lock state (PKS-07).
//!
//! Planned: logind `LockedHint` of the service's session plus
//! `org.freedesktop.ScreenSaver.ActiveChanged` (and `org.gnome.ScreenSaver`);
//! locked when any source says so. Until then the screen is never locked.

use tokio::sync::watch;

use super::LockMonitor;

/// The screen is never reported locked.
#[derive(Debug)]
pub struct NoLockMonitor {
    state: watch::Sender<bool>,
}

impl NoLockMonitor {
    /// Always unlocked.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: watch::Sender::new(false),
        }
    }
}

impl Default for NoLockMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl LockMonitor for NoLockMonitor {
    fn locked(&self) -> watch::Receiver<bool> {
        self.state.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}
