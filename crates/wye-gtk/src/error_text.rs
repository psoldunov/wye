//! What a window's banner says for a failed call: a [`Kind`] that decides
//! the sentence, and the detail the service sent.
//!
//! Mirrors crates/wye-ui/src/error_text.rs and the sentences of
//! crates/wye-ui/qml/components/WyeErrorText.qml; keep the three in step.

use wye_api::Error;

use crate::settings::save::CHANGED_ELSEWHERE;

/// Why a call failed, as far as the words for it go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// The detail is the whole message.
    #[default]
    Plain,
    /// `ReadOnly`: the configuration file cannot be written.
    ReadOnly,
    /// `NotLossless`: saving would drop values the file holds.
    NotLossless,
    /// `Conflict`: the configuration changed since the window read it.
    Conflict,
    /// `Conflict` from `crate::settings::save`: a list the change replaces
    /// changed elsewhere, so the change was not saved.
    ChangedElsewhere,
    /// `InvalidArgs`: the service refused what was sent.
    Refused,
    /// `Unavailable`: this session cannot do it.
    Unavailable,
    /// A bus error: the service cannot be reached.
    Unreachable,
    /// A history entry that is no longer there (DLG-HIS-03).
    EntryGone,
}

/// A failed call, for a banner.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ErrorText {
    pub kind: Kind,
    /// The service's own words, or the whole message.
    pub detail: String,
}

impl ErrorText {
    /// A message already worded (a local check's), shown as it is.
    #[must_use]
    pub fn plain(detail: impl Into<String>) -> Self {
        Self {
            kind: Kind::Plain,
            detail: detail.into(),
        }
    }

    /// Whether there is nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kind == Kind::Plain && self.detail.is_empty()
    }

    /// The sentence the banner shows.
    #[must_use]
    pub fn sentence(&self) -> String {
        let with_detail = |sentence: &str| {
            if self.detail.is_empty() {
                sentence.to_owned()
            } else {
                format!("{sentence} {}", self.detail)
            }
        };
        match self.kind {
            Kind::Plain => self.detail.clone(),
            Kind::ReadOnly => with_detail("Wye cannot change this setting: the file is read-only."),
            Kind::NotLossless => with_detail(
                "Wye did not save the change because it would drop values the configuration file contains.",
            ),
            Kind::Conflict => {
                "The configuration changed while you were editing it. Try again.".to_owned()
            }
            Kind::ChangedElsewhere => "This list changed elsewhere, so Wye did not save your change. It now shows the list as it is; try again.".to_owned(),
            Kind::Refused => format!("The service refused the change: {}", self.detail),
            Kind::Unavailable => format!("Not available in this session: {}", self.detail),
            // The detail is the bus's own error ("org.freedesktop.DBus.Error.
            // NoReply: …"), which says nothing to a reader; the log has it.
            Kind::Unreachable => {
                "Cannot reach the Wye service. Try again in a moment.".to_owned()
            }
            Kind::EntryGone => "That entry is gone.".to_owned(),
        }
    }
}

/// `error` for a banner.
#[must_use]
pub fn describe(error: &Error) -> ErrorText {
    let (kind, detail) = match error {
        Error::ReadOnly(reason) => (Kind::ReadOnly, reason.clone()),
        Error::NotLossless(reason) => (Kind::NotLossless, reason.clone()),
        Error::Conflict(reason) if reason == CHANGED_ELSEWHERE => {
            (Kind::ChangedElsewhere, String::new())
        }
        Error::Conflict(_) => (Kind::Conflict, String::new()),
        Error::InvalidArgs(reason) => (Kind::Refused, reason.clone()),
        Error::Unavailable(reason) => (Kind::Unavailable, reason.clone()),
        Error::NotFound(reason)
        | Error::Failed(reason)
        | Error::NotImplemented(reason)
        | Error::ScriptSyntax(reason) => (Kind::Plain, reason.clone()),
        Error::Bus(error) => (Kind::Unreachable, error.to_string()),
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
                kind: Kind::ReadOnly,
                detail: "home-manager".into()
            }
        );
        assert_eq!(
            describe(&Error::Conflict("stale".into())).kind,
            Kind::Conflict
        );
        assert_eq!(
            describe(&Error::NotFound("no previous browser".into())),
            ErrorText::plain("no previous browser"),
            "shown as the service words it"
        );
    }

    #[test]
    fn set_06_a_refused_list_change_says_so() {
        let text = describe(&Error::Conflict(CHANGED_ELSEWHERE.into()));
        assert_eq!(text.kind, Kind::ChangedElsewhere);
        assert!(text.sentence().starts_with("This list changed elsewhere"));
    }

    #[test]
    fn sentences_carry_the_detail_where_they_should() {
        let read_only = describe(&Error::ReadOnly("managed by Nix".into()));
        assert_eq!(
            read_only.sentence(),
            "Wye cannot change this setting: the file is read-only. managed by Nix"
        );
        let refused = describe(&Error::InvalidArgs("bad key".into()));
        assert_eq!(
            refused.sentence(),
            "The service refused the change: bad key"
        );
        assert!(ErrorText::default().is_empty());
        assert!(!ErrorText::plain("x").is_empty());
    }

    #[test]
    fn set_06_an_unreachable_service_is_a_sentence_not_the_bus_error() {
        let bus = zbus::Error::Failure(
            "org.freedesktop.DBus.Error.NoReply: Message recipient disconnected".into(),
        );
        let text = describe(&Error::Bus(bus));
        assert_eq!(text.kind, Kind::Unreachable);
        assert!(text.detail.contains("NoReply"), "kept for the log");
        assert_eq!(
            text.sentence(),
            "Cannot reach the Wye service. Try again in a moment."
        );
    }
}
