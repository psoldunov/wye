//! `PickerHost1.ShowPicker`: everything the picker draws (02-picker.md,
//! 07-picker-settings.md, 15-keyboard.md).
//!
//! The service decides the tiles, hotkeys and grouping; the UI only renders
//! them and sends the choice back with `PickerChose`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::context::Modifier;
use crate::{Badge, TargetCapabilities, TargetSpec};

/// One picker request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerRequest {
    /// The link (PICK-04).
    pub url: PickerUrl,
    /// The app the link came from (PICK-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PickerSource>,
    /// Tiles in order (PICK-06).
    pub tiles: Vec<PickerTile>,
    /// Overflow menus grouped by browser (TGT-02).
    pub overflow: Vec<OverflowGroup>,
    /// Appearance (PKS-01 to PKS-04).
    pub settings: PickerSettings,
    /// Key bindings (KEY-03, KEY-22).
    pub keys: PickerKeys,
    /// Modifiers already held when the picker opens (KEY-06).
    pub held: Vec<Modifier>,
    /// Where to open it; centred on the active output when absent (PICK-02).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    /// Opened from Settings to preview; choosing opens nothing (PKS-06).
    pub preview: bool,
}

/// The link, split for display.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerUrl {
    /// The whole URL.
    pub full: String,
    /// Host, shown emphasised.
    pub host: String,
    /// Everything after the host.
    pub rest: String,
}

/// The source app as shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerSource {
    /// Display name.
    pub name: String,
    /// Icon theme name or absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// One tile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerTile {
    /// What `PickerChose` sends back.
    pub target: TargetSpec,
    /// Display name.
    pub name: String,
    /// Icon theme name or absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Profile badge (DISC-08).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<Badge>,
    /// Hotkey label (KEY-10, KEY-11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    /// What the target can do.
    pub capabilities: TargetCapabilities,
}

/// A group in the overflow menu, usually one browser and its profiles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverflowGroup {
    /// Group label.
    pub label: String,
    /// Icon theme name or absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Entries.
    pub tiles: Vec<PickerTile>,
}

/// Picker appearance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerSettings {
    /// Tile icon size (PKS-01).
    pub icon_size: IconSize,
    /// Show names under icons (PKS-02).
    pub show_names: bool,
    /// Show the URL line (PKS-03).
    pub show_url: bool,
    /// Show profile badges (PKS-04).
    pub show_badge: bool,
}

wire_enum! {
    /// Tile icon size.
    #[derive(Default)]
    pub enum IconSize as "icon size" {
        Small = "small",
        #[default]
        Medium = "medium",
        Large = "large",
    }
}

/// The picker's key bindings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickerKeys {
    /// Action name to key names, for example `"open": ["Return"]` (KEY-03).
    pub actions: BTreeMap<String, Vec<String>>,
    /// Action name to the modifiers that trigger it while choosing, for
    /// example `"private": ["Shift"]` (KEY-22).
    pub modifier_actions: BTreeMap<String, Vec<Modifier>>,
}

/// Where the picker opens: the pointer on a named output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Placement {
    /// Output (connector) name, for example `DP-1`.
    pub output: String,
    /// Logical x on that output.
    pub x: i32,
    /// Logical y on that output.
    pub y: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_object_is_a_valid_request() {
        let request: PickerRequest = serde_json::from_str("{}").expect("decodes");
        assert_eq!(request.settings.icon_size, IconSize::Medium);
        assert!(request.placement.is_none());
    }

    #[test]
    fn keys_serialise_as_maps() {
        let keys = PickerKeys {
            actions: BTreeMap::from([("open".into(), vec!["Return".into()])]),
            modifier_actions: BTreeMap::from([("private".into(), vec![Modifier::Shift])]),
        };
        let json = serde_json::to_value(&keys).expect("encodes");
        assert_eq!(
            json,
            serde_json::json!({"actions": {"open": ["Return"]}, "modifierActions": {"private": ["Shift"]}})
        );
    }
}
