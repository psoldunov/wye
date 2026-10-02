//! What the script editor opens (SCR-01): the scope from `ShowWindow`'s
//! argument, the window title, and the self-test's stand-in data.
//!
//! The argument is a scope (`global`, `rule:<id>`) as the service sends it,
//! or JSON `{"scope": …, "ruleName": …, "fixture": …}` from the rule editor
//! (which knows the name of a rule not saved yet) and the self-test.

use serde::Deserialize;
use wye_api::actions::ScriptScope;
use wye_api::scripts::ScriptRun;

/// The title's prefix (SCR-01).
const TITLE: &str = "Transform Script";

/// One opening of the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub scope: ScriptScope,
    /// The rule's name, when the caller knows it.
    pub rule_name: Option<String>,
    /// Canned data instead of the service (`--self-test`).
    pub fixture: Option<Fixture>,
}

/// What the self-test shows instead of asking the service.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fixture {
    /// `GetScript`.
    pub source: String,
    /// The test link.
    #[serde(default)]
    pub test_url: Option<String>,
    /// `RunScript`'s answer.
    #[serde(default)]
    pub run: Option<ScriptRun>,
    /// Pretend the file changed on disk (SCR-08).
    #[serde(default)]
    pub external_change: bool,
    /// Source apps for the popup (SCR-04): desktop ID and name.
    #[serde(default)]
    pub apps: Vec<(String, String)>,
    /// Open the Reference as well (SCR-06). Only the window reads it, from
    /// the argument's JSON; it is here so the argument still parses.
    #[serde(default)]
    pub reference: bool,
    /// Text the window types over the loaded source, as the user would, to
    /// show unsaved changes. Only the window reads it, as `reference`.
    #[serde(default)]
    pub edit: Option<String>,
    /// Close the window once it is opened and edited, to show the question
    /// about unsaved changes (SCR-10). Only the window reads it, as `reference`.
    #[serde(default)]
    pub confirm_close: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JsonArgument {
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    rule_name: Option<String>,
    #[serde(default)]
    fixture: Option<Fixture>,
}

/// Read `ShowWindow("script-editor", argument)`. An empty argument means
/// the global script.
///
/// # Errors
///
/// A message for an unknown scope or JSON that does not fit.
pub fn parse(argument: &str) -> Result<Opening, String> {
    let argument = argument.trim();
    if argument.starts_with('{') {
        let json: JsonArgument = serde_json::from_str(argument)
            .map_err(|error| format!("the editor's argument is not valid: {error}"))?;
        return Ok(Opening {
            scope: scope(json.scope.as_deref().unwrap_or_default())?,
            rule_name: json.rule_name.filter(|name| !name.is_empty()),
            fixture: json.fixture,
        });
    }
    Ok(Opening {
        scope: scope(argument)?,
        rule_name: None,
        fixture: None,
    })
}

fn scope(text: &str) -> Result<ScriptScope, String> {
    if text.is_empty() {
        return Ok(ScriptScope::Global);
    }
    text.parse()
        .map_err(|error: wye_api::UnknownValue| error.to_string())
}

/// What the title calls a rule with no name: one the rule editor has not
/// saved and the user has not named yet, as that sheet's own title (RUL-10).
const UNNAMED_RULE: &str = "New Rule";

/// "Transform Script — Global" or "Transform Script — <rule name>" (SCR-01).
/// A rule's internal ID never shows: without a name it reads "New Rule"
/// until the configuration names it.
#[must_use]
pub fn title(scope: &ScriptScope, rule_name: Option<&str>) -> String {
    match (scope, rule_name) {
        (ScriptScope::Global, _) => format!("{TITLE} \u{2014} Global"),
        (ScriptScope::Rule(_), Some(name)) => format!("{TITLE} \u{2014} {name}"),
        (ScriptScope::Rule(_), None) => format!("{TITLE} \u{2014} {UNNAMED_RULE}"),
    }
}

/// The name of the rule with ID `id` in `GetConfig`'s JSON.
#[must_use]
pub fn rule_name(config_json: &str, id: &str) -> Option<String> {
    let config: serde_json::Value = serde_json::from_str(config_json).ok()?;
    config
        .get("rules")?
        .as_array()?
        .iter()
        .find(|rule| rule.get("id").and_then(serde_json::Value::as_str) == Some(id))?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_scopes_come_from_the_service() {
        assert_eq!(parse("global").expect("valid").scope, ScriptScope::Global);
        assert_eq!(parse("").expect("valid").scope, ScriptScope::Global);
        assert_eq!(
            parse("rule:gh").expect("valid").scope,
            ScriptScope::Rule("gh".into())
        );
        assert!(parse("rule:").is_err());
        assert!(parse("nonsense").is_err());
    }

    #[test]
    fn json_arguments_carry_a_name_and_a_fixture() {
        let opening = parse(
            r#"{"scope": "rule:gh", "ruleName": "GitHub", "fixture": {"source": "x", "externalChange": true}}"#,
        )
        .expect("valid");
        assert_eq!(opening.rule_name.as_deref(), Some("GitHub"));
        let fixture = opening.fixture.expect("fixture");
        assert_eq!(fixture.source, "x");
        assert!(fixture.external_change);
        assert!(parse(r#"{"scope": "global", "colour": 1}"#).is_err());
    }

    #[test]
    fn scr_01_titles_name_the_script() {
        assert_eq!(
            title(&ScriptScope::Global, None),
            "Transform Script \u{2014} Global"
        );
        let rule = ScriptScope::Rule("gh".into());
        assert_eq!(
            title(&rule, Some("GitHub")),
            "Transform Script \u{2014} GitHub"
        );
        // An unsaved, unnamed rule: never its ID (SCR-01, RUL-10).
        let unsaved = ScriptScope::Rule("rule-1a0fc71c4fe".into());
        assert_eq!(title(&unsaved, None), "Transform Script \u{2014} New Rule");
    }

    #[test]
    fn rule_names_are_found_by_id() {
        let config = r#"{"rules": [{"name": "A"}, {"id": "gh", "name": "GitHub"}]}"#;
        assert_eq!(rule_name(config, "gh").as_deref(), Some("GitHub"));
        assert_eq!(rule_name(config, "zz"), None);
        assert_eq!(rule_name("not json", "gh"), None);
    }
}
