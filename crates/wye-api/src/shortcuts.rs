//! `GetShortcuts`: global shortcuts and how they are provided (KEY-40,
//! KEY-41, ADV-05 to ADV-07).

use serde::{Deserialize, Serialize};

/// Action ID of the toggle-menu shortcut (TRAY-08).
pub const TOGGLE_MENU: &str = "toggle-menu";
/// Action ID of "open the clipboard URL in the primary browser" (IN-03).
pub const CLIPBOARD_PRIMARY: &str = "clipboard-primary";
/// Action ID of "open the clipboard URL in the alternative browser" (IN-04).
pub const CLIPBOARD_ALTERNATIVE: &str = "clipboard-alternative";

/// Global shortcuts as the session reports them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shortcuts {
    /// How shortcuts are registered.
    pub mechanism: ShortcutMechanism,
    /// One entry per action.
    pub bindings: Vec<ShortcutBinding>,
}

wire_enum! {
    /// How global shortcuts are registered.
    #[derive(Default)]
    pub enum ShortcutMechanism as "shortcut mechanism" {
        /// The `GlobalShortcuts` portal.
        Portal = "portal",
        /// X11 key grabs.
        X11 = "x11",
        /// None: the user binds the commands in the desktop's settings
        /// (KEY-41).
        #[default]
        None = "none",
    }
}

/// One shortcut action.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutBinding {
    /// Action ID, for example [`TOGGLE_MENU`].
    pub action: String,
    /// What the action does, in words.
    pub description: String,
    /// The trigger as reported, for example `Ctrl+Alt+W`; absent when
    /// unbound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// The command that does the same, for binding by hand (KEY-41), for
    /// example `wye menu`.
    pub command: String,
}
