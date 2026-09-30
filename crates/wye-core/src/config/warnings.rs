//! Problems with the configuration that Wye works around.

use std::fmt;

use crate::rule::RuleError;

/// A problem with the configuration that Wye works around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    UnknownKey(String),
    /// A value of the wrong type or shape. The key keeps its default; the
    /// rest of the file still applies.
    InvalidValue {
        key: String,
        error: String,
    },
    /// An entry of a list (a rule, a shown browser) that cannot be read;
    /// only that entry is skipped.
    InvalidEntry {
        key: String,
        index: usize,
        error: String,
    },
    /// The values that could be read did not combine into a configuration;
    /// the defaults apply.
    Unusable(String),
    /// `browsers.primary` or `browsers.alternative` is `default`, which only
    /// means something in mappings and rules. Treated as the picker.
    DefaultNotAllowed(&'static str),
    /// A shown entry that is the picker or Default; dropped.
    ShownNotConcrete(usize),
    DuplicateHotkey(String),
    HotkeyTakenByAction(String),
    /// KEY-11: a target hotkey is one key without modifiers.
    InvalidHotkey(String),
    /// KEY-21: two held-modifier picker actions share a modifier set.
    ModifierClash {
        first: &'static str,
        second: &'static str,
    },
    UnknownService(String),
    InvalidRule {
        index: usize,
        name: String,
        errors: Vec<RuleError>,
    },
    DuplicateRuleId(String),
    OutOfRange {
        key: &'static str,
        value: String,
        used: String,
    },
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(path) => write!(f, "unknown key `{path}` is ignored"),
            Self::InvalidValue { key, error } => {
                write!(f, "`{key}` is ignored and keeps its default: {error}")
            }
            Self::InvalidEntry { key, index, error } => {
                write!(f, "`{key}` entry {} is skipped: {error}", index + 1)
            }
            Self::Unusable(error) => {
                write!(
                    f,
                    "the configuration cannot be used ({error}); using defaults"
                )
            }
            Self::DefaultNotAllowed(key) => write!(
                f,
                "`{key}` cannot be `default` (it only applies to web app mappings and rules); using the picker"
            ),
            Self::ShownNotConcrete(i) => write!(
                f,
                "shown browser {} is the picker or Default, which the picker cannot show; it is skipped",
                i + 1
            ),
            Self::DuplicateHotkey(key) => write!(
                f,
                "picker hotkey {key:?} is assigned more than once; only the first counts"
            ),
            Self::HotkeyTakenByAction(key) => write!(
                f,
                "picker hotkey {key:?} is already a picker action key; it is ignored"
            ),
            Self::InvalidHotkey(key) => write!(
                f,
                "picker hotkey {key:?} is not a single key without modifiers; it is ignored"
            ),
            Self::ModifierClash { first, second } => write!(
                f,
                "`{first}` and `{second}` use the same modifiers; the picker cannot tell them apart"
            ),
            Self::UnknownService(id) => write!(
                f,
                "`apps.{id}` does not name a known web app; it is ignored"
            ),
            Self::InvalidRule {
                index,
                name,
                errors,
            } => {
                let errors: Vec<_> = errors.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "rule {} ({name:?}) is skipped: {}",
                    index + 1,
                    errors.join("; ")
                )
            }
            Self::DuplicateRuleId(id) => write!(f, "rule ID {id:?} is used more than once"),
            Self::OutOfRange { key, value, used } => {
                write!(f, "`{key}` = {value} is out of range; using {used}")
            }
        }
    }
}
