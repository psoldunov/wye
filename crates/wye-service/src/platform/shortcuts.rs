//! Global shortcuts (KEY-40, KEY-41, ADV-05 to ADV-07).
//!
//! Planned: the `GlobalShortcuts` portal through ashpd
//! (`register_host_app("dev.soldunov.wye")`, one long-lived session). Until
//! then there is no mechanism and the user binds `wye menu` and `wye
//! clipboard` by hand (KEY-41).

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{BoundShortcut, PlatformError, ShortcutProvider};

/// How many presses a slow listener may fall behind.
const ACTIVATION_BUFFER: usize = 16;

/// No global shortcut mechanism.
#[derive(Debug)]
pub struct NoShortcuts {
    activations: broadcast::Sender<String>,
}

impl NoShortcuts {
    /// Never activates.
    #[must_use]
    pub fn new() -> Self {
        Self {
            activations: broadcast::Sender::new(ACTIVATION_BUFFER),
        }
    }
}

impl Default for NoShortcuts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ShortcutProvider for NoShortcuts {
    async fn bindings(&self) -> Result<Vec<BoundShortcut>, PlatformError> {
        Ok(Vec::new())
    }

    async fn bind(&self, _action: &str, _trigger: &str) -> Result<(), PlatformError> {
        Err(unavailable())
    }

    async fn configure(&self) -> Result<(), PlatformError> {
        Err(unavailable())
    }

    fn activations(&self) -> broadcast::Receiver<String> {
        self.activations.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

fn unavailable() -> PlatformError {
    PlatformError::Unavailable("no global shortcut mechanism in this session".to_owned())
}
