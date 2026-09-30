//! The Keyboard Shortcuts group of the Advanced page (ADV-05 to ADV-07,
//! KEY-40, KEY-41): one row per global action with the binding the
//! mechanism reports, or the command to bind by hand where there is no
//! mechanism.

use serde::Serialize;
use serde_json::Value;
use wye_api::shortcuts::{
    CLIPBOARD_ALTERNATIVE, CLIPBOARD_PRIMARY, ShortcutMechanism, Shortcuts, TOGGLE_MENU,
};

use super::record;

/// The actions in the order of the page, with their names on it and the CLI
/// command that does the same (KEY-41).
const ACTIONS: [(&str, &str, &str); 3] = [
    (TOGGLE_MENU, "Toggle menu", "wye menu"),
    (
        CLIPBOARD_PRIMARY,
        "Open URL from clipboard with primary browser",
        "wye clipboard",
    ),
    (
        CLIPBOARD_ALTERNATIVE,
        "Open URL from clipboard with alternative browser",
        "wye clipboard --alternative",
    ),
];

/// One row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub action: String,
    pub title: String,
    /// The binding: what the mechanism reports, else what the configuration
    /// holds; empty for none.
    pub binding: String,
    /// The binding as it reads in the row (KEY-03).
    pub label: String,
    /// The command to bind in the compositor (KEY-41).
    pub command: String,
}

/// What the group shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// `portal`, `x11` or `none`.
    pub mechanism: String,
    /// Recording works: the session has a mechanism.
    pub recordable: bool,
    /// The mechanism has its own dialog: "Change…" (KEY-40).
    pub configurable: bool,
    pub rows: Vec<Row>,
}

fn row(action: &str, title: &str, command: &str, binding: Option<&str>) -> Row {
    let binding = binding.unwrap_or_default().to_owned();
    let label = record::labels(std::slice::from_ref(&binding))
        .into_iter()
        .next()
        .filter(|_| !binding.is_empty())
        .unwrap_or_default();
    Row {
        action: action.to_owned(),
        title: title.to_owned(),
        binding,
        label,
        command: command.to_owned(),
    }
}

fn configured(config: &Value, action: &str) -> Option<String> {
    config
        .pointer(&format!("/shortcuts/{action}"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The group from `GetShortcuts`. The configuration's own binding fills in
/// when the mechanism reports none (KEY-40).
#[must_use]
pub fn from_wire(wire: &Shortcuts, config: &Value) -> View {
    let recordable = wire.mechanism != ShortcutMechanism::None;
    let rows = ACTIONS
        .iter()
        .map(|(action, title, command)| {
            let bound = wire
                .bindings
                .iter()
                .find(|binding| binding.action == *action);
            let trigger = bound.and_then(|binding| binding.trigger.clone());
            let command = bound
                .map(|binding| binding.command.as_str())
                .filter(|text| !text.is_empty())
                .unwrap_or(command);
            row(
                action,
                title,
                command,
                trigger.or_else(|| configured(config, action)).as_deref(),
            )
        })
        .collect();
    View {
        mechanism: wire.mechanism.as_str().to_owned(),
        recordable,
        configurable: wire.mechanism == ShortcutMechanism::Portal,
        rows,
    }
}

/// The group when the service cannot say (a call that fails, or one not
/// implemented yet): no mechanism, so the commands to bind by hand
/// (KEY-41).
#[must_use]
pub fn fallback(config: &Value) -> View {
    from_wire(&Shortcuts::default(), config)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wye_api::shortcuts::ShortcutBinding;

    use super::*;

    fn wire(mechanism: ShortcutMechanism, trigger: Option<&str>) -> Shortcuts {
        Shortcuts {
            mechanism,
            bindings: vec![ShortcutBinding {
                action: TOGGLE_MENU.to_owned(),
                description: "Toggle menu".to_owned(),
                trigger: trigger.map(str::to_owned),
                command: "wye menu".to_owned(),
            }],
        }
    }

    #[test]
    fn the_rows_follow_the_page_order() {
        // ADV-05 to ADV-07
        let view = from_wire(&wire(ShortcutMechanism::Portal, None), &json!({}));
        let actions: Vec<_> = view.rows.iter().map(|row| row.action.as_str()).collect();
        assert_eq!(
            actions,
            [TOGGLE_MENU, CLIPBOARD_PRIMARY, CLIPBOARD_ALTERNATIVE]
        );
        assert_eq!(view.rows[0].title, "Toggle menu");
    }

    #[test]
    fn a_portal_row_shows_the_trigger_the_portal_reports() {
        // KEY-40
        let view = from_wire(
            &wire(ShortcutMechanism::Portal, Some("CTRL+ALT+O")),
            &json!({"shortcuts": {"toggle-menu": "Ctrl+o"}}),
        );
        assert_eq!(view.rows[0].binding, "CTRL+ALT+O");
        assert!(view.recordable && view.configurable);
    }

    #[test]
    fn without_a_trigger_the_configuration_fills_in() {
        let view = from_wire(
            &wire(ShortcutMechanism::X11, None),
            &json!({"shortcuts": {"toggle-menu": "Ctrl+Shift+o"}}),
        );
        assert_eq!(view.rows[0].binding, "Ctrl+Shift+o");
        assert_eq!(view.rows[0].label, "Ctrl+Shift+O");
        assert!(view.recordable && !view.configurable);
    }

    #[test]
    fn an_unset_row_has_no_binding_and_no_label() {
        // ADV-05: unset by default
        let view = from_wire(&wire(ShortcutMechanism::X11, None), &json!({}));
        assert_eq!(
            (view.rows[1].binding.as_str(), view.rows[1].label.as_str()),
            ("", "")
        );
    }

    #[test]
    fn no_mechanism_shows_the_commands_to_bind_by_hand() {
        // KEY-41
        let view = fallback(&json!({}));
        assert!(!view.recordable);
        let commands: Vec<_> = view.rows.iter().map(|row| row.command.as_str()).collect();
        assert_eq!(
            commands,
            ["wye menu", "wye clipboard", "wye clipboard --alternative"]
        );
    }
}
