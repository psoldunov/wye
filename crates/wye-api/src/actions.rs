//! Names passed as `s` arguments: application actions, windows, picker
//! actions, history reopen modes, tray hosts and script scopes.

use std::fmt;
use std::str::FromStr;

use crate::UnknownValue;

wire_enum! {
    /// `org.freedesktop.Application.ActivateAction` names; the desktop
    /// entry's actions use the same names.
    pub enum ApplicationAction as "action" {
        Settings = "settings",
        Clipboard = "clipboard",
        ClipboardAlternative = "clipboard-alternative",
        Menu = "menu",
        Setup = "setup",
        History = "history",
        TestRules = "test-rules",
        About = "about",
        Quit = "quit",
    }
}

wire_enum! {
    /// The `window` argument of `ShowWindow`. The argument's meaning depends
    /// on the window: a page for `settings`, a scope for `script-editor`, a
    /// JSON prefill for `rule-editor`, empty otherwise.
    pub enum Window as "window" {
        Settings = "settings",
        FirstRun = "first-run",
        History = "history",
        TestRules = "test-rules",
        About = "about",
        ScriptEditor = "script-editor",
        RuleEditor = "rule-editor",
    }
}

wire_enum! {
    /// The `action` argument of `PickerAction`.
    pub enum PickerAction as "picker action" {
        /// Copy the link and close the picker.
        CopyLink = "copy-link",
        /// Open the rule editor pre-filled; the link is not opened (PICK-31).
        CreateRule = "create-rule",
    }
}

wire_enum! {
    /// The `how` argument of `ReopenHistoryEntry` (DLG-HIS-03, TRAY-15).
    pub enum Reopen as "reopen mode" {
        Picker = "picker",
        SameTarget = "same-target",
    }
}

wire_enum! {
    /// The `kind` argument of `RegisterTray`.
    pub enum TrayHost as "tray host" {
        PlasmaApplet = "plasma-applet",
        GnomeExtension = "gnome-extension",
    }
}

/// Prefix of a rule's script scope.
pub const RULE_SCOPE_PREFIX: &str = "rule:";

/// Which script `GetScript`, `SetScript` and `ScriptFileChanged` mean:
/// `global` or `rule:<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptScope {
    /// The global transform (`transform.js`).
    Global,
    /// One rule's transform (`rules/<id>.js`).
    Rule(String),
}

impl fmt::Display for ScriptScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => formatter.write_str("global"),
            Self::Rule(id) => write!(formatter, "{RULE_SCOPE_PREFIX}{id}"),
        }
    }
}

impl FromStr for ScriptScope {
    type Err = UnknownValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.strip_prefix(RULE_SCOPE_PREFIX) {
            _ if value == "global" => Ok(Self::Global),
            Some(id) if !id.is_empty() => Ok(Self::Rule(id.to_owned())),
            _ => Err(UnknownValue {
                kind: "script scope",
                value: value.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_scopes_round_trip() {
        for scope in [ScriptScope::Global, ScriptScope::Rule("gh-1".into())] {
            assert_eq!(scope.to_string().parse::<ScriptScope>(), Ok(scope));
        }
    }

    #[test]
    fn a_rule_scope_needs_an_id() {
        assert!("rule:".parse::<ScriptScope>().is_err());
        assert!("rules".parse::<ScriptScope>().is_err());
    }

    #[test]
    fn window_names_match_the_contract() {
        assert_eq!(Window::FirstRun.as_str(), "first-run");
        assert_eq!("script-editor".parse(), Ok(Window::ScriptEditor));
    }

    #[test]
    fn every_action_parses_its_own_spelling() {
        for action in ApplicationAction::ALL {
            assert_eq!(action.as_str().parse::<ApplicationAction>(), Ok(*action));
        }
    }
}
