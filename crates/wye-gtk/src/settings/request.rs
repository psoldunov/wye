//! What a `ShowWindow` for the Settings window asks for (SET-04, SET-08).
//!
//! The argument is a page name (`rules`), empty for the last page, or, from
//! `wye-gtk --self-test`, a JSON object `{page, fixture, scheme, sheet}`
//! (`fixtures/settings.json`): `fixture` is what the service would have
//! returned (`super::fixture`), `sheet` a sheet to open on its page; such a
//! case starts from the window alone (`case`). For
//! `rule-editor` and `test-rules` the argument belongs to the Rules page (a
//! rule editor prefill) and the page is always Rules.
//!
//! Same reading as `handle()` in crates/wye-ui/qml/settings/SettingsWindow.qml.

use serde_json::Value;

/// The window names this surface takes.
#[cfg(test)]
pub const SETTINGS: &str = "settings";
pub const RULE_EDITOR: &str = "rule-editor";
pub const TEST_RULES: &str = "test-rules";

/// The page the rule editor and the tester live on.
pub const RULES_PAGE: &str = "rules";

/// One `ShowWindow` for Settings, read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Request {
    /// The page to show; `None` for the last one (SET-08).
    pub page: Option<String>,
    /// Service data to show instead of asking the service (self-test).
    pub fixture: Option<Value>,
    /// A sheet to open on its page (self-test).
    pub sheet: Option<String>,
    /// A self-test case (a JSON object): it starts with no sheet or popover
    /// open, whatever the case before left.
    pub case: bool,
    /// For `rule-editor` / `test-rules`: the window name and its argument,
    /// for the Rules page.
    pub rules: Option<(String, String)>,
}

impl Request {
    /// Read `ShowWindow(key, argument)`.
    #[must_use]
    pub fn parse(key: &str, argument: &str) -> Self {
        if key == RULE_EDITOR || key == TEST_RULES {
            return Self {
                page: Some(RULES_PAGE.to_owned()),
                rules: Some((key.to_owned(), argument.to_owned())),
                ..Self::default()
            };
        }
        let text = argument.trim();
        if !text.starts_with('{') {
            return Self {
                page: Some(text.to_owned()).filter(|page| !page.is_empty()),
                ..Self::default()
            };
        }
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text) else {
            tracing::warn!(%argument, "the settings argument is not a JSON object");
            return Self::default();
        };
        let string = |name: &str| {
            object
                .get(name)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        Self {
            page: string("page"),
            fixture: object.get("fixture").cloned(),
            sheet: string("sheet"),
            rules: None,
            case: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn set_04_a_page_name_or_nothing() {
        assert_eq!(
            Request::parse(SETTINGS, "rules").page.as_deref(),
            Some("rules")
        );
        assert_eq!(
            Request::parse(SETTINGS, "").page,
            None,
            "SET-08: the last page"
        );
    }

    #[test]
    fn a_self_test_object_carries_its_parts() {
        let request = Request::parse(
            SETTINGS,
            r#"{"page":"general","scheme":"dark","sheet":"x","fixture":{"revision":3}}"#,
        );
        assert_eq!(request.page.as_deref(), Some("general"));
        assert_eq!(request.sheet.as_deref(), Some("x"));
        assert_eq!(request.fixture, Some(json!({"revision": 3})));
        assert!(request.case);
        assert!(
            !Request::parse(SETTINGS, "rules").case,
            "a page name is not a case"
        );
    }

    #[test]
    fn the_rule_editor_opens_on_rules_with_its_prefill() {
        let prefill = r#"{"domain":"example.com","sourceApp":null}"#;
        let request = Request::parse(RULE_EDITOR, prefill);
        assert_eq!(request.page.as_deref(), Some(RULES_PAGE));
        assert_eq!(
            request.rules,
            Some((RULE_EDITOR.to_owned(), prefill.to_owned()))
        );
        assert_eq!(request.fixture, None, "a prefill is not a fixture");
    }

    #[test]
    fn broken_json_is_the_last_page() {
        assert_eq!(Request::parse(SETTINGS, "{nope"), Request::default());
    }
}
