//! `GetTargets`: every target the user can pick (TGT-02 to TGT-07,
//! SHOWN-02, PICK-06, DISC-08).

use serde::{Deserialize, Serialize};

use crate::{Badge, Packaging, TargetCapabilities, TargetSpec};

/// The target inventory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TargetInventory {
    /// Every target: the Picker, then apps, private windows, profiles and
    /// custom apps, in TGT-02 order.
    pub targets: Vec<TargetInfo>,
}

/// One target and how to show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    /// The target in the configuration's shape.
    pub target: TargetSpec,
    /// Which variant this is.
    pub kind: TargetKind,
    /// Display name, for example "Work (Chrome)".
    pub name: String,
    /// Name without the browser, for example "Work" (TRAY-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Icon theme name or absolute path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Profile badge (DISC-08).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge: Option<Badge>,
    /// Desktop ID of the browser a private window or profile belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// What the target can do.
    #[serde(default)]
    pub capabilities: TargetCapabilities,
    /// How the app is installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packaging: Option<Packaging>,
    /// The app is gone; the target is kept and falls back to the Picker
    /// (APP-10).
    #[serde(default)]
    pub missing: bool,
}

wire_enum! {
    /// The variants of a target (12-data-model.md, "Target").
    pub enum TargetKind as "target kind" {
        Picker = "picker",
        Default = "default",
        App = "app",
        Private = "private",
        Profile = "profile",
        Custom = "custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_target_round_trips() {
        let info = TargetInfo {
            target: serde_json::json!({"profile": {"app": "google-chrome.desktop", "id": "Profile 1"}}),
            kind: TargetKind::Profile,
            name: "Work (Chrome)".into(),
            short_name: Some("Work".into()),
            icon: Some("google-chrome".into()),
            badge: Some(Badge::Initial {
                initial: "W".into(),
                color: "#336699".into(),
            }),
            browser: Some("google-chrome.desktop".into()),
            capabilities: TargetCapabilities::default(),
            packaging: Some(Packaging::Native),
            missing: false,
        };
        let text = serde_json::to_string(&info).expect("encodes");
        assert!(text.contains(r#""shortName":"Work""#), "{text}");
        assert_eq!(
            serde_json::from_str::<TargetInfo>(&text).expect("decodes"),
            info
        );
    }
}
