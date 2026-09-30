//! Names of the transient systemd scopes launched apps are moved into
//! (LAUNCH-06).
//!
//! The name follows the desktop-launcher convention systemd documents for
//! desktop environments, `app-<launcher>-<ApplicationID>-<RANDOM>.scope`,
//! with the application ID escaped so that `-` only ever separates the
//! parts. Plasma and GNOME name their scopes the same way, and
//! [`crate::source_app::parse_app_unit`] reads them back.

use std::fmt::Write as _;

use wye_core::DesktopId;

/// The launcher part of the unit name.
pub const LAUNCHER: &str = "wye";

/// systemd refuses unit names longer than this.
const MAX_UNIT_NAME: usize = 255;

/// The scope unit for one launch of `app`: its desktop ID, or the program
/// for a custom executable. `random` keeps two launches of the same app
/// apart.
#[must_use]
pub fn unit_name(app: &ScopeApp<'_>, random: u64) -> String {
    let prefix = format!("app-{LAUNCHER}-");
    let suffix = format!("-{random:x}.scope");
    let budget = MAX_UNIT_NAME.saturating_sub(prefix.len() + suffix.len());
    format!("{prefix}{}{suffix}", escape(app.application_id(), budget))
}

/// What a scope is named after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeApp<'a> {
    /// A desktop entry: its ID without `.desktop`.
    Desktop(&'a DesktopId),
    /// A custom executable: its file name.
    Program(&'a str),
}

impl ScopeApp<'_> {
    fn application_id(&self) -> &str {
        match self {
            Self::Desktop(id) => id.app_id(),
            Self::Program(program) => program.rsplit('/').next().unwrap_or(program),
        }
    }
}

/// systemd unit-name escaping: ASCII letters, digits, `_`, `.` and `:`
/// stay, anything else (and a leading `.`) becomes `\xNN` per byte. Stops
/// before an escape that would exceed `budget` bytes.
fn escape(id: &str, budget: usize) -> String {
    let mut escaped = String::new();
    for (index, ch) in id.char_indices() {
        let keep =
            ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':') || (ch == '.' && index > 0);
        let piece = if keep {
            ch.to_string()
        } else {
            let mut bytes = [0; 4];
            ch.encode_utf8(&mut bytes)
                .bytes()
                .fold(String::new(), |mut piece, byte| {
                    let _ = write!(piece, "\\x{byte:02x}");
                    piece
                })
        };
        if escaped.len() + piece.len() > budget {
            break;
        }
        escaped.push_str(&piece);
    }
    if escaped.is_empty() {
        "app".to_owned()
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_app::parse_app_unit;

    fn id(value: &str) -> DesktopId {
        DesktopId::new(value).expect("valid desktop ID")
    }

    #[test]
    fn a_desktop_id_names_the_scope() {
        let firefox = id("firefox.desktop");
        assert_eq!(
            unit_name(&ScopeApp::Desktop(&firefox), 0xbeef),
            "app-wye-firefox-beef.scope"
        );
    }

    #[test]
    fn dashes_and_other_characters_are_escaped() {
        let chrome = id("google-chrome.desktop");
        assert_eq!(
            unit_name(&ScopeApp::Desktop(&chrome), 1),
            "app-wye-google\\x2dchrome-1.scope"
        );
        assert_eq!(
            unit_name(&ScopeApp::Program("/opt/my browser/run"), 2),
            "app-wye-run-2.scope"
        );
        assert_eq!(escape(".hidden é", 100), "\\x2ehidden\\x20\\xc3\\xa9");
    }

    #[test]
    fn the_name_reads_back_as_the_same_app() {
        for value in ["org.mozilla.firefox.desktop", "google-chrome.desktop"] {
            let app = id(value);
            let unit = unit_name(&ScopeApp::Desktop(&app), 0x1234);
            assert_eq!(parse_app_unit(&unit), Some(app), "{unit}");
        }
    }

    #[test]
    fn long_names_stay_within_the_systemd_limit() {
        let long = "a-".repeat(200);
        let unit = unit_name(&ScopeApp::Program(&long), u64::MAX);
        assert!(unit.len() <= MAX_UNIT_NAME, "{}", unit.len());
        assert!(unit.ends_with(&format!("-{:x}.scope", u64::MAX)));
        assert!(
            !unit.contains("\\x2\\"),
            "an escape was cut in half: {unit}"
        );
    }
}
