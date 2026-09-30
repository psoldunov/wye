//! The `Status` property: default-browser state, configuration health,
//! session capabilities, lock state and UI state.

use serde::{Deserialize, Serialize};

use crate::AppRef;

/// Everything a frontend shows about the service's situation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    /// Default-browser registration (DEF-02, DEF-03, ONB-11).
    pub default_browser: DefaultBrowserStatus,
    /// The configuration file.
    pub config: ConfigStatus,
    /// What this session supports (KEY-06, DLG-ABT-02).
    pub capabilities: Capabilities,
    /// The screen is locked (PKS-07).
    pub locked: bool,
    /// Starting at login is managed outside Wye (the Nix modules set
    /// `WYE_LOGIN_MANAGED=1`): the service never writes the autostart entry,
    /// and `general.launch-at-login` changes nothing (GEN-01).
    pub login_managed: bool,
    /// State the UI keeps across runs; changed with `UpdateUiState`.
    pub ui_state: UiState,
}

/// Whether Wye is the default browser, and what else is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DefaultBrowserStatus {
    /// Wye handles `https` links.
    pub is_default: bool,
    /// The app that handles `https` links now, when it is not Wye.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<AppRef>,
    /// The browser Wye replaced and restores (DEF-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<AppRef>,
    /// The user chose to keep another default (ONB-11).
    pub kept_current: bool,
}

/// The configuration file's health.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigStatus {
    /// Absolute path of `config.toml`.
    pub path: String,
    /// The file can be written (false for a home-manager symlink).
    pub writable: bool,
    /// Saving would keep every value the file contains.
    pub lossless: bool,
    /// Problems found while loading, one line each.
    pub warnings: Vec<String>,
    /// Why the file on disk is not in use; the last good configuration is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What this session supports. `None` means unavailable; otherwise the
/// mechanism in use, for example `"wayland-layer-shell"` or `"portal"`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Capabilities {
    /// Held-modifier detection (KEY-06).
    pub held_keys: Option<String>,
    /// Pointer position for placing the picker (PICK-02).
    pub pointer: Option<String>,
    /// Source-app fallbacks beyond the process chain, in order.
    pub source_app_fallbacks: Vec<String>,
    /// Reading the clipboard on demand (IN-02, TRAY-10).
    pub clipboard_read: Option<String>,
    /// Watching the clipboard (EXT-12).
    pub clipboard_watch: Option<String>,
    /// Writing the clipboard back (EXT-12).
    pub clipboard_write: Option<String>,
    /// Global shortcuts (KEY-40).
    pub global_shortcuts: Option<String>,
    /// Screen-lock detection (PKS-07).
    pub lock_detection: Option<String>,
    /// The picker can be a layer-shell overlay.
    pub layer_shell: bool,
}

/// State the UI keeps across runs. `UpdateUiState` takes a JSON merge patch
/// of this shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    /// First run finished (ONB-06).
    pub onboarding_done: bool,
    /// IDs of callouts the user dismissed (BLK-09).
    pub dismissed_callouts: Vec<String>,
    /// Settings page shown last (SET-08).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_page: Option<String>,
    /// The rules help arrow was seen (RUL-19).
    pub help_arrow_seen: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_object_is_the_default_status() {
        let status: Status = serde_json::from_str("{}").expect("decodes");
        assert_eq!(status, Status::default());
    }

    #[test]
    fn keys_are_camel_case() {
        let json = serde_json::to_value(Status::default()).expect("encodes");
        assert!(json.get("defaultBrowser").is_some());
        assert_eq!(json["loginManaged"], serde_json::Value::Bool(false));
        assert!(json["uiState"].get("onboardingDone").is_some());
        assert!(json["capabilities"].get("heldKeys").is_some());
    }
}
