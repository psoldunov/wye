//! Types several payloads share.

use serde::{Deserialize, Serialize};

/// A target in the configuration's JSON shape, for example
/// `{"app": "firefox.desktop"}`, `{"picker": true}`, `{"private":
/// "firefox.desktop"}` or `{"profile": {"app": "google-chrome.desktop", "id":
/// "Profile 1"}}` (12-data-model.md, "Target").
///
/// Kept as JSON so this crate does not depend on `wye-core`; the service
/// converts it with `wye_core::Target`'s serde implementation. The same text
/// is what the `target` string argument of `PickerChose` and `SetPrimary`
/// carries.
pub type TargetSpec = serde_json::Value;

/// An installed app, as far as a frontend needs to show it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    /// Desktop ID, for example `firefox.desktop`.
    pub id: String,
    /// Display name (the entry's localised `Name`).
    pub name: String,
    /// Icon theme name or absolute path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

wire_enum! {
    /// How an app is installed.
    pub enum Packaging as "packaging" {
        Native = "native",
        Flatpak = "flatpak",
        Snap = "snap",
    }
}

/// A profile badge (DISC-08): the profile's picture, or an initial on a
/// colour. The UI draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Badge {
    /// An image file.
    Image {
        /// Absolute path.
        image: String,
    },
    /// A letter on a coloured disc.
    Initial {
        /// One character.
        initial: String,
        /// `#rrggbb`.
        color: String,
    },
}

/// What a target can do when opening a link.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCapabilities {
    /// Can open a private window.
    pub private: bool,
    /// Can force a new window.
    pub new_window: bool,
    /// Can open in the background.
    pub background: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_serialise_as_either_shape() {
        let image = Badge::Image {
            image: "/tmp/a.png".into(),
        };
        let initial = Badge::Initial {
            initial: "W".into(),
            color: "#336699".into(),
        };
        assert_eq!(
            serde_json::to_string(&image).expect("encodes"),
            r#"{"image":"/tmp/a.png"}"#
        );
        assert_eq!(
            serde_json::to_string(&initial).expect("encodes"),
            r##"{"initial":"W","color":"#336699"}"##
        );
        for badge in [image, initial] {
            let text = serde_json::to_string(&badge).expect("encodes");
            assert_eq!(
                serde_json::from_str::<Badge>(&text).expect("decodes"),
                badge
            );
        }
    }

    #[test]
    fn capabilities_use_camel_case_keys() {
        let text = serde_json::to_string(&TargetCapabilities {
            private: true,
            new_window: true,
            background: false,
        })
        .expect("encodes");
        assert_eq!(
            text,
            r#"{"private":true,"newWindow":true,"background":false}"#
        );
    }

    #[test]
    fn packaging_parses_its_own_spelling() {
        for packaging in Packaging::ALL {
            assert_eq!(packaging.as_str().parse::<Packaging>(), Ok(*packaging));
        }
        assert!("deb".parse::<Packaging>().is_err());
    }
}
