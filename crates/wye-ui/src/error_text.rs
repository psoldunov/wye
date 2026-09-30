//! What a window's message bar shows for a failed call: a kind, which QML
//! words itself so the sentence can be translated
//! (`qml/components/WyeErrorText.qml`), and the detail the service sent.
//!
//! A backend publishes both as its `errorKind` and `error` properties. A
//! message with no kind is shown as it is.

use wye_api::Error;

use crate::settings::save::CHANGED_ELSEWHERE;

/// `ReadOnly`: the configuration file cannot be written.
pub const READ_ONLY: &str = "read-only";
/// `NotLossless`: saving would drop values the file holds.
pub const NOT_LOSSLESS: &str = "not-lossless";
/// `Conflict`: the configuration changed since the window read it.
pub const CONFLICT: &str = "conflict";
/// `Conflict` from [`crate::settings::save`]: a list the change replaces
/// changed elsewhere, so the change was not saved.
pub const CHANGED_ELSEWHERE_KIND: &str = "changed-elsewhere";
/// `InvalidArgs`: the service refused what was sent.
pub const REFUSED: &str = "refused";
/// `Unavailable`: this session cannot do it.
pub const UNAVAILABLE: &str = "unavailable";
/// A bus error: the service cannot be reached.
pub const UNREACHABLE: &str = "unreachable";
/// A history entry that is no longer there (DLG-HIS-03).
pub const ENTRY_GONE: &str = "entry-gone";

/// A failed call, for the message bar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ErrorText {
    /// One of the constants above; empty when `detail` is the whole message.
    pub kind: &'static str,
    /// The service's own words, or the whole message.
    pub detail: String,
}

impl ErrorText {
    /// A message already worded (a local check's), shown as it is.
    #[must_use]
    pub fn plain(detail: impl Into<String>) -> Self {
        Self {
            kind: "",
            detail: detail.into(),
        }
    }

    /// A message with a kind and no detail.
    #[must_use]
    pub const fn kind(kind: &'static str) -> Self {
        Self {
            kind,
            detail: String::new(),
        }
    }
}

/// `error` for the message bar.
#[must_use]
pub fn describe(error: &Error) -> ErrorText {
    let (kind, detail) = match error {
        Error::ReadOnly(reason) => (READ_ONLY, reason.clone()),
        Error::NotLossless(reason) => (NOT_LOSSLESS, reason.clone()),
        Error::Conflict(reason) if reason == CHANGED_ELSEWHERE => {
            (CHANGED_ELSEWHERE_KIND, String::new())
        }
        Error::Conflict(_) => (CONFLICT, String::new()),
        Error::InvalidArgs(reason) => (REFUSED, reason.clone()),
        Error::Unavailable(reason) => (UNAVAILABLE, reason.clone()),
        Error::NotFound(reason)
        | Error::Failed(reason)
        | Error::NotImplemented(reason)
        | Error::ScriptSyntax(reason) => ("", reason.clone()),
        Error::Bus(error) => (UNREACHABLE, error.to_string()),
    };
    ErrorText { kind, detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_errors_become_kinds_with_their_detail() {
        assert_eq!(
            describe(&Error::ReadOnly("home-manager".into())),
            ErrorText {
                kind: READ_ONLY,
                detail: "home-manager".into()
            }
        );
        assert_eq!(describe(&Error::Conflict("stale".into())).kind, CONFLICT);
        assert_eq!(
            describe(&Error::NotFound("no previous browser".into())),
            ErrorText {
                kind: "",
                detail: "no previous browser".into()
            },
            "shown as the service words it"
        );
    }

    #[test]
    fn set_06_a_refused_list_change_says_so() {
        let text = describe(&Error::Conflict(CHANGED_ELSEWHERE.into()));
        assert_eq!(text, ErrorText::kind(CHANGED_ELSEWHERE_KIND));
    }
}
