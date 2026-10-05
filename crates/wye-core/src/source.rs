//! Source apps: the app in which the user clicked a link.

use std::ffi::OsStr;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::target::{CustomApp, DesktopId, Target};

/// What Wye managed to learn about the app that opened a link. Either part
/// may be missing: a Flatpak app behind the `OpenURI` portal often yields
/// nothing at all ([13-linux-platform.md](../../../docs/spec/13-linux-platform.md)).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SourceApp {
    /// Desktop ID, from the caller's systemd scope or
    /// `GIO_LAUNCHED_DESKTOP_FILE`.
    pub desktop_id: Option<DesktopId>,
    /// File name of the caller's executable, such as `slack`.
    pub executable: Option<String>,
}

impl SourceApp {
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        self.desktop_id.is_none() && self.executable.is_none()
    }

    /// True when `target` opens this very app, not one of its profiles or
    /// private windows (DEF-08).
    ///
    /// Desktop IDs compare ignoring ASCII case, because an ID derived from
    /// the focused window may differ in case from the entry's (`Figma.desktop`
    /// and `figma.desktop`). When only the executable is known, it is
    /// compared with the application ID, again ignoring case (the window
    /// class `Figma` for `figma.desktop`). A program path matches by its file
    /// name. An unknown source is never an app.
    #[must_use]
    pub fn is_app(&self, target: &Target) -> bool {
        match target {
            Target::App(id) | Target::Custom(CustomApp::Desktop(id)) => {
                match (&self.desktop_id, &self.executable) {
                    (Some(own), _) => own.as_str().eq_ignore_ascii_case(id.as_str()),
                    (None, Some(executable)) => executable.eq_ignore_ascii_case(id.app_id()),
                    (None, None) => false,
                }
            }
            Target::Custom(CustomApp::Executable(path)) => {
                let name = Path::new(path).file_name().and_then(OsStr::to_str);
                name.is_some() && name == self.executable.as_deref()
            }
            Target::Picker | Target::Default | Target::Private(_) | Target::Profile { .. } => false,
        }
    }
}

impl fmt::Display for SourceApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.desktop_id, &self.executable) {
            (Some(id), _) => write!(f, "{id}"),
            (None, Some(exe)) => f.write_str(exe),
            (None, None) => f.write_str("unknown"),
        }
    }
}

/// A source-app condition in a rule: a desktop ID (preferred) or an
/// executable name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum SourceAppSpec {
    Desktop(DesktopId),
    Executable(String),
}

impl SourceAppSpec {
    /// True when the detected source is this app. An unknown source never
    /// matches (13-linux-platform.md: "Rules with source apps simply do not
    /// match when the source is unknown").
    #[must_use]
    pub fn matches(&self, source: &SourceApp) -> bool {
        match self {
            Self::Desktop(id) => source.desktop_id.as_ref() == Some(id),
            Self::Executable(name) => source.executable.as_deref() == Some(name.as_str()),
        }
    }
}

impl From<String> for SourceAppSpec {
    fn from(value: String) -> Self {
        let trimmed = value.trim();
        match DesktopId::new(trimmed) {
            Ok(id) if trimmed.ends_with(DesktopId::SUFFIX) => Self::Desktop(id),
            _ => Self::Executable(trimmed.to_owned()),
        }
    }
}

impl From<SourceAppSpec> for String {
    fn from(value: SourceAppSpec) -> Self {
        match value {
            SourceAppSpec::Desktop(id) => id.into(),
            SourceAppSpec::Executable(name) => name,
        }
    }
}

impl fmt::Display for SourceAppSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Desktop(id) => write!(f, "{id}"),
            Self::Executable(name) => f.write_str(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slack() -> SourceApp {
        SourceApp {
            desktop_id: Some(DesktopId::new("com.slack.Slack.desktop").unwrap()),
            executable: Some("slack".into()),
        }
    }

    #[test]
    fn spec_matches_by_desktop_id_or_executable() {
        assert!(SourceAppSpec::from("com.slack.Slack.desktop".to_owned()).matches(&slack()));
        assert!(SourceAppSpec::from("slack".to_owned()).matches(&slack()));
        assert!(!SourceAppSpec::from("discord.desktop".to_owned()).matches(&slack()));
    }

    #[test]
    fn unknown_source_never_matches() {
        assert!(!SourceAppSpec::from("slack".to_owned()).matches(&SourceApp::default()));
    }

    fn desktop(id: &str) -> Target {
        Target::App(DesktopId::new(id).unwrap())
    }

    fn custom_desktop(id: &str) -> Target {
        Target::Custom(CustomApp::Desktop(DesktopId::new(id).unwrap()))
    }

    fn source(desktop_id: Option<&str>, executable: Option<&str>) -> SourceApp {
        SourceApp {
            desktop_id: desktop_id.map(|id| DesktopId::new(id).unwrap()),
            executable: executable.map(str::to_owned),
        }
    }

    // DEF-08
    #[test]
    fn is_app_compares_desktop_ids_ignoring_case() {
        let figma = source(Some("figma.desktop"), None);
        assert!(figma.is_app(&desktop("figma.desktop")));
        assert!(figma.is_app(&custom_desktop("figma.desktop")));
        assert!(source(Some("Figma.desktop"), None).is_app(&custom_desktop("figma.desktop")));
        assert!(!figma.is_app(&desktop("linear.desktop")));
    }

    // DEF-08
    #[test]
    fn is_app_falls_back_to_the_executable_without_a_desktop_id() {
        let class = source(None, Some("Figma"));
        assert!(class.is_app(&custom_desktop("figma.desktop")));
        assert!(class.is_app(&desktop("figma.desktop")));
        assert!(!class.is_app(&desktop("linear.desktop")));
        // A known desktop ID decides; the executable is not consulted.
        assert!(!source(Some("linear.desktop"), Some("figma")).is_app(&desktop("figma.desktop")));
    }

    // DEF-08
    #[test]
    fn is_app_matches_a_program_by_file_name() {
        let figma = source(None, Some("figma"));
        let program = |path: &str| Target::Custom(CustomApp::Executable(path.to_owned()));
        assert!(figma.is_app(&program("/opt/figma/figma")));
        assert!(figma.is_app(&program("figma")));
        assert!(!figma.is_app(&program("/opt/figma/Figma")));
        assert!(!figma.is_app(&program("/opt/figma/other")));
        assert!(!source(Some("figma.desktop"), None).is_app(&program("/opt/figma/figma")));
    }

    // DEF-08: profiles and private windows are browser targets, not the app.
    #[test]
    fn is_app_is_false_for_everything_else() {
        let firefox = source(Some("firefox.desktop"), Some("firefox"));
        let profile = Target::Profile {
            app: DesktopId::new("firefox.desktop").unwrap(),
            id: "work".into(),
        };
        let private = Target::Private(DesktopId::new("firefox.desktop").unwrap());
        for target in [Target::Picker, Target::Default, profile, private] {
            assert!(!firefox.is_app(&target), "{target}");
        }
        assert!(!SourceApp::default().is_app(&desktop("firefox.desktop")));
        assert!(!SourceApp::default().is_app(&Target::Custom(CustomApp::Executable("x".into()))));
    }
}
