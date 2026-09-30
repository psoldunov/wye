//! Source apps: the app in which the user clicked a link.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::target::DesktopId;

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
}
