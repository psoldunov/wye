//! Targets: the places a link can open ([12-data-model.md](../../../docs/spec/12-data-model.md)).

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A freedesktop desktop-file ID such as `firefox.desktop` or
/// `app.zen_browser.zen.desktop`.
///
/// Targets store desktop IDs rather than paths, so they survive package
/// updates and moves between `/usr`, `/nix/store` and Flatpak exports.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DesktopId(String);

impl DesktopId {
    pub const SUFFIX: &'static str = ".desktop";

    /// Builds an ID, appending `.desktop` when it is missing (systemd scopes
    /// and Flatpak app IDs carry the bare application ID).
    ///
    /// # Errors
    ///
    /// Returns an error for an empty ID or one containing a path separator or
    /// whitespace.
    pub fn new(id: impl Into<String>) -> Result<Self, InvalidDesktopId> {
        let mut id = id.into();
        let stem = id.strip_suffix(Self::SUFFIX).unwrap_or(&id);
        if stem.is_empty() || id.contains('/') || id.chars().any(char::is_whitespace) {
            return Err(InvalidDesktopId(id));
        }
        if !id.ends_with(Self::SUFFIX) {
            id.push_str(Self::SUFFIX);
        }
        Ok(Self(id))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The ID without its `.desktop` suffix, which is the application ID used
    /// in D-Bus names, systemd units and Flatpak.
    #[must_use]
    pub fn app_id(&self) -> &str {
        self.0.strip_suffix(Self::SUFFIX).unwrap_or(&self.0)
    }
}

impl fmt::Display for DesktopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DesktopId {
    type Error = InvalidDesktopId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DesktopId> for String {
    fn from(value: DesktopId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a desktop ID (expected something like firefox.desktop)")]
pub struct InvalidDesktopId(String);

/// Where a link opens.
///
/// In the configuration file a target is a one-key table:
/// `{ picker = true }`, `{ default = true }`, `{ app = "firefox.desktop" }`,
/// `{ private = "firefox.desktop" }`,
/// `{ profile = { app = "google-chrome.desktop", id = "Profile 1" } }` or
/// `{ custom = "/opt/tool/bin/tool" }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// Ask the user with the picker.
    Picker,
    /// Follow the primary browser. Only valid in web app mappings and rules.
    Default,
    /// An installed app that handles `http`/`https` links.
    App(DesktopId),
    /// A browser's private window.
    Private(DesktopId),
    /// A browser profile. `id` is the profile directory (Chromium) or the
    /// profile path from `profiles.ini` (Firefox).
    Profile { app: DesktopId, id: String },
    /// Any other app the user added, by desktop ID or executable path.
    Custom(CustomApp),
}

impl Target {
    /// The desktop entry that launches this target, if it has one.
    #[must_use]
    pub fn desktop_id(&self) -> Option<&DesktopId> {
        match self {
            Self::App(id)
            | Self::Private(id)
            | Self::Profile { app: id, .. }
            | Self::Custom(CustomApp::Desktop(id)) => Some(id),
            Self::Picker | Self::Default | Self::Custom(CustomApp::Executable(_)) => None,
        }
    }

    /// True for targets that name a concrete app, as opposed to Picker and
    /// Default.
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        !matches!(self, Self::Picker | Self::Default)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Picker => f.write_str("Picker"),
            Self::Default => f.write_str("Default"),
            Self::App(id) => write!(f, "{id}"),
            Self::Private(id) => write!(f, "{id} (private)"),
            Self::Profile { app, id } => write!(f, "{app} (profile {id:?})"),
            Self::Custom(app) => write!(f, "{app}"),
        }
    }
}

/// An app added through "Other…": a desktop entry or a bare executable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CustomApp {
    Desktop(DesktopId),
    Executable(String),
}

impl CustomApp {
    /// A value ending in `.desktop` is a desktop ID; anything else is an
    /// executable path or name.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty value or an invalid desktop ID.
    pub fn parse(value: &str) -> Result<Self, InvalidDesktopId> {
        let value = value.trim();
        if value.ends_with(DesktopId::SUFFIX) {
            DesktopId::new(value).map(Self::Desktop)
        } else if value.is_empty() {
            Err(InvalidDesktopId(String::new()))
        } else {
            Ok(Self::Executable(value.to_owned()))
        }
    }
}

impl fmt::Display for CustomApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Desktop(id) => write!(f, "{id}"),
            Self::Executable(path) => f.write_str(path),
        }
    }
}

/// The on-disk shape of a [`Target`]: an externally tagged one-key table.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum TargetRepr {
    Picker(bool),
    Default(bool),
    App(DesktopId),
    Private(DesktopId),
    Profile { app: DesktopId, id: String },
    Custom(String),
}

impl Serialize for Target {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self.clone() {
            Self::Picker => TargetRepr::Picker(true),
            Self::Default => TargetRepr::Default(true),
            Self::App(id) => TargetRepr::App(id),
            Self::Private(id) => TargetRepr::Private(id),
            Self::Profile { app, id } => TargetRepr::Profile { app, id },
            Self::Custom(app) => TargetRepr::Custom(app.to_string()),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        Ok(match TargetRepr::deserialize(deserializer)? {
            TargetRepr::Picker(true) => Self::Picker,
            TargetRepr::Default(true) => Self::Default,
            TargetRepr::Picker(false) | TargetRepr::Default(false) => {
                return Err(D::Error::custom(
                    "`picker` and `default` targets take the value `true`",
                ));
            }
            TargetRepr::App(id) => Self::App(id),
            TargetRepr::Private(id) => Self::Private(id),
            TargetRepr::Profile { app, id } => Self::Profile { app, id },
            TargetRepr::Custom(value) => {
                Self::Custom(CustomApp::parse(&value).map_err(D::Error::custom)?)
            }
        })
    }
}

/// Answers whether a target can open right now. Implemented over the
/// discovered desktop entries and profiles; the pipeline uses it to fall back
/// when a configured app was uninstalled.
pub trait Availability {
    fn is_available(&self, target: &Target) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Holder {
        t: Target,
    }

    fn parse(toml_src: &str) -> Result<Target, toml::de::Error> {
        toml::from_str::<Holder>(toml_src).map(|h| h.t)
    }

    #[test]
    fn desktop_id_appends_suffix() {
        assert_eq!(
            DesktopId::new("com.slack.Slack").unwrap().as_str(),
            "com.slack.Slack.desktop"
        );
        assert_eq!(
            DesktopId::new("firefox.desktop").unwrap().app_id(),
            "firefox"
        );
        assert!(DesktopId::new("").is_err());
        assert!(DesktopId::new(".desktop").is_err());
        assert!(DesktopId::new("a/b.desktop").is_err());
    }

    #[test]
    fn parses_every_target_shape() {
        assert_eq!(parse("t = { picker = true }").unwrap(), Target::Picker);
        assert_eq!(parse("t = { default = true }").unwrap(), Target::Default);
        assert_eq!(
            parse(r#"t = { app = "firefox.desktop" }"#).unwrap(),
            Target::App(DesktopId::new("firefox.desktop").unwrap())
        );
        assert_eq!(
            parse(r#"t = { private = "firefox.desktop" }"#).unwrap(),
            Target::Private(DesktopId::new("firefox.desktop").unwrap())
        );
        assert_eq!(
            parse(r#"t = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }"#)
                .unwrap(),
            Target::Profile {
                app: DesktopId::new("google-chrome.desktop").unwrap(),
                id: "Profile 1".into()
            }
        );
        assert_eq!(
            parse(r#"t = { custom = "/opt/tool/bin/tool" }"#).unwrap(),
            Target::Custom(CustomApp::Executable("/opt/tool/bin/tool".into()))
        );
        assert_eq!(
            parse(r#"t = { custom = "org.example.Tool.desktop" }"#).unwrap(),
            Target::Custom(CustomApp::Desktop(
                DesktopId::new("org.example.Tool.desktop").unwrap()
            ))
        );
    }

    #[test]
    fn rejects_malformed_targets() {
        assert!(parse("t = { picker = false }").is_err());
        assert!(parse(r#"t = { app = "firefox.desktop", private = "x.desktop" }"#).is_err());
        assert!(parse(r#"t = { browser = "firefox.desktop" }"#).is_err());
        assert!(parse(r#"t = { app = "" }"#).is_err());
    }

    #[test]
    fn round_trips() {
        for target in [
            Target::Picker,
            Target::Default,
            Target::App(DesktopId::new("firefox.desktop").unwrap()),
            Target::Profile {
                app: DesktopId::new("chromium.desktop").unwrap(),
                id: "Default".into(),
            },
            Target::Custom(CustomApp::Executable("tool".into())),
        ] {
            let text = toml::to_string(&Holder { t: target.clone() }).unwrap();
            assert_eq!(parse(&text).unwrap(), target, "{text}");
        }
    }
}
