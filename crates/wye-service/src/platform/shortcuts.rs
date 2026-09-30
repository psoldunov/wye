//! Global shortcuts (KEY-40, KEY-41, ADV-05 to ADV-07).
//!
//! The `GlobalShortcuts` portal ([`portal::PortalShortcuts`]) where the
//! session has one; otherwise there is no mechanism ([`NoShortcuts`]) and the
//! user binds `wye menu` and `wye clipboard` in the compositor's settings
//! (KEY-41). X11 key grabs without a portal are for later.

pub mod portal;
pub mod trigger;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use wye_api::shortcuts::{CLIPBOARD_ALTERNATIVE, CLIPBOARD_PRIMARY, TOGGLE_MENU};

pub use self::portal::{PortalShortcuts, Preferred};
use super::{BoundShortcut, PlatformError, ShortcutProvider};

/// How many presses a slow listener may fall behind.
const ACTIVATION_BUFFER: usize = 16;

/// One action a global shortcut can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutAction {
    /// Action ID, for example [`TOGGLE_MENU`].
    pub id: &'static str,
    /// What it does, as the desktop's shortcut settings list it.
    pub description: &'static str,
    /// The command that does the same, to bind by hand (KEY-41).
    pub command: &'static str,
}

/// Every action, in the order Settings lists them (ADV-05 to ADV-07).
pub const ACTIONS: [ShortcutAction; 3] = [
    ShortcutAction {
        id: TOGGLE_MENU,
        description: "Toggle the Wye menu",
        command: "wye menu",
    },
    ShortcutAction {
        id: CLIPBOARD_PRIMARY,
        description: "Open URL from clipboard with primary browser",
        command: "wye clipboard",
    },
    ShortcutAction {
        id: CLIPBOARD_ALTERNATIVE,
        description: "Open URL from clipboard with alternative browser",
        command: "wye clipboard --alternative",
    },
];

/// The action with ID `id`.
#[must_use]
pub fn action(id: &str) -> Option<&'static ShortcutAction> {
    ACTIONS.iter().find(|action| action.id == id)
}

/// The stored bindings of `shortcuts` by action ID.
#[must_use]
pub fn preferred(shortcuts: &wye_core::config::Shortcuts) -> Preferred {
    [
        (TOGGLE_MENU, &shortcuts.toggle_menu),
        (CLIPBOARD_PRIMARY, &shortcuts.clipboard_primary),
        (CLIPBOARD_ALTERNATIVE, &shortcuts.clipboard_alternative),
    ]
    .into_iter()
    .filter_map(|(id, stored)| {
        stored
            .as_deref()
            .filter(|stored| !stored.trim().is_empty())
            .map(|stored| (id, stored.to_owned()))
    })
    .collect()
}

/// The `[shortcuts]` table of the configuration file at `path`; nothing
/// preferred when it is missing or unreadable (the service reports that
/// elsewhere).
pub async fn preferred_in_file(path: &Path) -> Preferred {
    let text = match tokio::fs::read_to_string(path).await {
        Ok(text) => text,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, path = %path.display(), "cannot read the configuration");
            }
            return Preferred::new();
        }
    };
    let shortcuts = toml::from_str::<toml::Table>(&text)
        .ok()
        .and_then(|mut table| table.remove("shortcuts"))
        .map(toml::Value::try_into::<wye_core::config::Shortcuts>)
        .transpose()
        .inspect_err(|error| tracing::debug!(%error, "cannot read [shortcuts]"))
        .ok()
        .flatten()
        .unwrap_or_default();
    preferred(&shortcuts)
}

/// The best mechanism of this session: the portal on a connection of its
/// own, else none (KEY-41).
pub async fn detect(config: Option<&Path>) -> Arc<dyn ShortcutProvider> {
    let preferred = match config {
        Some(path) => preferred_in_file(path).await,
        None => Preferred::new(),
    };
    let connection = match zbus::Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            tracing::info!(%error, "no session bus for global shortcuts");
            return Arc::new(NoShortcuts::new());
        }
    };
    match PortalShortcuts::start(connection, preferred).await {
        Ok(portal) => Arc::new(portal),
        Err(error) => {
            tracing::info!(%error, "no global shortcuts portal");
            Arc::new(NoShortcuts::new())
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_its_command_key_41() {
        let commands: Vec<_> = ACTIONS.iter().map(|action| action.command).collect();
        assert_eq!(
            commands,
            ["wye menu", "wye clipboard", "wye clipboard --alternative"]
        );
        assert_eq!(action(TOGGLE_MENU).map(|a| a.command), Some("wye menu"));
        assert!(action("paste").is_none());
    }

    #[test]
    fn only_set_bindings_are_preferred_adv_05_to_07() {
        let shortcuts = wye_core::config::Shortcuts {
            toggle_menu: Some("Ctrl+Alt+w".into()),
            clipboard_primary: Some(String::new()),
            clipboard_alternative: None,
        };
        let preferred = preferred(&shortcuts);
        assert_eq!(
            preferred.into_iter().collect::<Vec<_>>(),
            [(TOGGLE_MENU, "Ctrl+Alt+w".to_owned())]
        );
    }

    #[tokio::test]
    async fn the_file_names_the_preferences() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[general]\nx = 1\n[shortcuts]\nclipboard-alternative = \"Super+v\"\n",
        )
        .expect("written");
        let preferred = preferred_in_file(&path).await;
        assert_eq!(
            preferred.get(CLIPBOARD_ALTERNATIVE).map(String::as_str),
            Some("Super+v")
        );
        assert!(
            preferred_in_file(&dir.path().join("missing.toml"))
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn without_a_mechanism_binding_is_unavailable_key_41() {
        let none = NoShortcuts::new();
        assert!(none.mechanism().is_none());
        assert!(none.bindings().await.expect("listed").is_empty());
        assert!(matches!(
            none.bind(TOGGLE_MENU, "Ctrl+w").await,
            Err(PlatformError::Unavailable(_))
        ));
    }
}
