//! The `Tray` property: the tray menu model (01-tray-menu.md).
//!
//! The `StatusNotifierItem` tray, the `wye-ui` popup (TRAY-08) and any
//! external tray host render the same model as-is.

use serde::{Deserialize, Serialize};

/// The tray icon and its menu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenu {
    /// What the icon shows (TRAY-02).
    pub icon: TrayIcon,
    /// Emblem over the icon (TRAY-18, ONB-11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<TrayOverlay>,
    /// Whether the icon is shown (TRAY-04).
    pub visible: bool,
    /// Menu items in order (menu layout table, TRAY-10 to TRAY-18).
    pub items: Vec<TrayItem>,
}

/// Wye's monochrome tray icon, which takes the panel's colour (GEN-02
/// "Wye").
pub const APP_ICON: &str = "dev.soldunov.wye-symbolic";

/// The picker glyph, a bulleted list (TRAY-02): the tray icon while the
/// primary browser is the Picker, and the Picker's menu item.
pub const PICKER_ICON: &str = "dev.soldunov.wye-picker-symbolic";

/// The tray icon (TRAY-02).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TrayIcon {
    /// Wye's own icon, [`APP_ICON`] (GEN-02 "Wye", or a primary browser
    /// with no icon).
    App,
    /// The picker glyph, [`PICKER_ICON`]: the primary browser is the
    /// Picker.
    #[default]
    Picker,
    /// An icon theme name or absolute path: the primary browser's icon.
    Theme {
        /// Theme name or path.
        name: String,
    },
}

impl TrayIcon {
    /// The icon theme name or path a host draws.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::App => APP_ICON,
            Self::Picker => PICKER_ICON,
            Self::Theme { name } => name,
        }
    }
}

wire_enum! {
    /// An emblem over the tray icon.
    pub enum TrayOverlay as "tray overlay" {
        /// Wye is not the default browser (TRAY-18).
        Warning = "warning",
    }
}

/// One menu entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItem {
    /// Stable ID the frontend sends back when the item is chosen.
    pub id: String,
    /// What kind of entry this is.
    pub kind: TrayItemKind,
    /// Text; empty for separators.
    #[serde(default)]
    pub label: String,
    /// Icon theme name or absolute path (TRAY-14); [`PICKER_ICON`] for the
    /// Picker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Shortcut shown next to the item, for example `P`, `1`, `Ctrl+,`
    /// (TRAY-13).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Whether the item can be chosen (TRAY-10).
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Radio state (TRAY-11).
    #[serde(default)]
    pub checked: bool,
    /// Entries of a submenu (TRAY-15).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TrayItem>,
}

const fn enabled_by_default() -> bool {
    true
}

wire_enum! {
    /// The kind of a menu entry.
    pub enum TrayItemKind as "tray item kind" {
        Action = "action",
        /// Non-interactive, dimmed section header.
        Header = "header",
        Radio = "radio",
        Separator = "separator",
        Submenu = "submenu",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_serialises_with_camel_case_and_tagged_icon() {
        let menu = TrayMenu {
            icon: TrayIcon::Theme {
                name: "firefox".into(),
            },
            overlay: Some(TrayOverlay::Warning),
            visible: true,
            items: vec![TrayItem {
                id: "primary:picker".into(),
                kind: TrayItemKind::Radio,
                label: "Picker".into(),
                icon: None,
                shortcut: Some("P".into()),
                enabled: true,
                checked: true,
                children: Vec::new(),
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&menu).expect("encodes");
        assert_eq!(
            json["icon"],
            serde_json::json!({"kind": "theme", "name": "firefox"})
        );
        assert_eq!(json["overlay"], "warning");
        assert_eq!(json["items"][0]["kind"], "radio");
        assert!(json["items"][0].get("children").is_none());
    }

    #[test]
    fn every_icon_names_what_to_draw() {
        assert_eq!(TrayIcon::App.name(), APP_ICON);
        assert_eq!(TrayIcon::Picker.name(), PICKER_ICON);
        let theme = TrayIcon::Theme {
            name: "firefox".into(),
        };
        assert_eq!(theme.name(), "firefox");
        let json = serde_json::to_value(TrayIcon::App).expect("encodes");
        assert_eq!(json, serde_json::json!({"kind": "app"}));
    }

    #[test]
    fn a_minimal_item_is_enabled_and_unchecked() {
        let item: TrayItem =
            serde_json::from_str(r#"{"id":"sep-1","kind":"separator"}"#).expect("decodes");
        assert!(item.enabled);
        assert!(!item.checked);
        assert!(item.label.is_empty());
    }
}
