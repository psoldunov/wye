//! A rule's transform script (RUL-25): **Edit Script…** asks the service to
//! show the script editor for `rule:<id>` (ADV-04's editor, through
//! `ShowWindow`, so whichever host serves it opens it); turning Transform
//! URL on for a script that does not exist yet opens it too (SCR-09).
//!
//! Same calls as `showWindow` and `openScriptIfMissing` in
//! crates/wye-ui/src/settings/pages.rs.

use serde_json::json;
use wye_api::Error;

use crate::settings::store::SettingsStore;

/// The window name of the script editor.
const SCRIPT_EDITOR: &str = "script-editor";

/// The script scope of rule `id`.
#[must_use]
pub fn scope(id: &str) -> String {
    format!("rule:{id}")
}

/// The editor's argument: the scope and, for its title, the rule's name.
fn argument(scope: &str, rule_name: &str) -> String {
    json!({"scope": scope, "ruleName": rule_name}).to_string()
}

/// Show the script editor for `scope`. Nothing under the self-test.
pub fn open(store: &SettingsStore, scope: &str, rule_name: &str) {
    if store.offline() {
        tracing::info!(%scope, "the script editor was asked for in the self-test");
        return;
    }
    let argument = argument(scope, rule_name);
    crate::service::request(
        move |proxy| async move { proxy.show_window(SCRIPT_EDITOR, &argument).await },
        shown,
    );
}

/// Log a `ShowWindow` that failed.
fn shown(result: Result<(), Error>) {
    if let Err(error) = result {
        tracing::warn!(%error, "cannot show the script editor");
    }
}

/// SCR-09: open the editor when the script for `scope` does not exist yet.
pub fn open_if_missing(store: &SettingsStore, scope: &str, rule_name: &str) {
    if store.offline() {
        return;
    }
    let (asked, argument) = (scope.to_owned(), argument(scope, rule_name));
    crate::service::request(
        move |proxy| async move { proxy.script_exists(&asked).await },
        move |result| match result {
            Ok(false) => crate::service::request(
                move |proxy| async move { proxy.show_window(SCRIPT_EDITOR, &argument).await },
                shown,
            ),
            Ok(true) => {}
            // The service cannot say: leave the editor closed.
            Err(error) => tracing::debug!(%error, "cannot tell whether the script exists"),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rul_25_a_rule_script_is_named_by_the_rule_id() {
        assert_eq!(scope("gh-1"), "rule:gh-1");
        let text = argument("rule:gh-1", "GitHub “work”");
        let value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(value["scope"], "rule:gh-1");
        assert_eq!(value["ruleName"], "GitHub “work”");
    }
}
