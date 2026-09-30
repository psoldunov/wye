//! The rule editor's draft (RUL-10 to RUL-28): a rule in its configuration
//! shape (kebab-case JSON), which the sheet edits field by field, checked
//! here before Save is enabled (RUL-18).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wye_core::matcher::UrlMatcher;
use wye_core::{Modifiers, Rule, Target};

/// What `ShowWindow("rule-editor", …)` asks for (PICK-31, DLG-TST-03).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefill {
    /// A Domain matcher for this host (PICK-31).
    #[serde(default)]
    pub domain: Option<String>,
    /// The source app's desktop ID.
    #[serde(default)]
    pub source_app: Option<String>,
    /// Edit the rule at this position instead of a new one.
    #[serde(default)]
    pub index: Option<usize>,
}

impl Prefill {
    /// Read the argument; anything unreadable is an empty new rule.
    #[must_use]
    pub fn parse(argument: &str) -> Self {
        serde_json::from_str(argument).unwrap_or_default()
    }
}

/// Whether the draft can be saved, and what to show where (RUL-14, RUL-18,
/// RUL-27).
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag marks one field of the editor that shows its own error"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub valid: bool,
    /// The name is empty.
    pub name_missing: bool,
    /// Neither a matcher, a source app nor held keys.
    pub no_condition: bool,
    /// One entry per matcher: its error, or empty.
    pub matcher_errors: Vec<String>,
    /// The held keys equal the alternative-browser key, which wins (RUL-27).
    pub alternative_key_wins: bool,
    /// The draft is not a rule at all (a broken target, say).
    pub error: String,
}

/// How many seeds [`fresh_id`] tries before it gives up on a unique ID.
const ID_ATTEMPTS: u64 = 4096;

/// An ID no rule in `rules` uses, derived from `seed` (the time, say), so
/// a new rule can own a script before it is saved (RUL-25).
#[must_use]
pub fn fresh_id(rules: &[Rule], seed: u64) -> String {
    let taken = |id: &str| rules.iter().any(|rule| rule.id.as_deref() == Some(id));
    (0..ID_ATTEMPTS)
        .map(|step: u64| format!("rule-{:x}", seed.wrapping_add(step)))
        .find(|id| !taken(id))
        .unwrap_or_else(|| "rule".to_owned())
}

/// A new rule (RUL-10, RUL-12): the default target, filled from `prefill`.
#[must_use]
pub fn new_draft(prefill: &Prefill, id: &str) -> Value {
    let matchers: Vec<Value> = prefill
        .domain
        .iter()
        .filter(|domain| !domain.is_empty())
        .map(|domain| json!({"kind": "domain", "pattern": domain}))
        .collect();
    let sources: Vec<&String> = prefill
        .source_app
        .iter()
        .filter(|app| !app.is_empty())
        .collect();
    json!({
        "id": id,
        "name": prefill.domain.clone().unwrap_or_default(),
        "enabled": true,
        "target": serde_json::to_value(Target::Default).unwrap_or(Value::Null),
        "url-matchers": matchers,
        "source-apps": sources,
        "held-keys": [],
        "open-in-background": false,
        "force-new-window": false,
        "run": "before",
        "transform": false,
    })
}

/// Rule `index` as a draft, with every field present, and an ID if it had
/// none; `None` when there is no such rule.
#[must_use]
pub fn draft_for(rules: &[Rule], index: usize, id_if_missing: &str) -> Option<Value> {
    let rule = rules.get(index)?;
    let rule = Rule {
        id: rule.id.clone().or_else(|| Some(id_if_missing.to_owned())),
        ..rule.clone()
    };
    let mut value = serde_json::to_value(&rule).ok()?;
    let object = value.as_object_mut()?;
    for key in ["url-matchers", "source-apps", "held-keys"] {
        object.entry(key).or_insert_with(|| json!([]));
    }
    Some(value)
}

/// The rule a draft describes.
///
/// # Errors
///
/// The reason the draft is not a rule.
pub fn rule_of(draft: &Value) -> Result<Rule, String> {
    serde_json::from_value(draft.clone()).map_err(|error| error.to_string())
}

/// RUL-18: a name, one condition, every matcher valid.
#[must_use]
pub fn check(draft: &Value, alternative_key: Modifiers) -> Check {
    let rule = match rule_of(draft) {
        Ok(rule) => rule,
        Err(error) => {
            return Check {
                error,
                ..Check::default()
            };
        }
    };
    let matcher_errors: Vec<String> = rule
        .url_matchers
        .iter()
        .map(|matcher| matcher_error(matcher).unwrap_or_default())
        .collect();
    let name_missing = rule.name.trim().is_empty();
    let no_condition =
        rule.url_matchers.is_empty() && rule.source_apps.is_empty() && rule.held_keys.is_empty();
    Check {
        valid: !name_missing && !no_condition && matcher_errors.iter().all(String::is_empty),
        name_missing,
        no_condition,
        matcher_errors,
        alternative_key_wins: !rule.held_keys.is_empty() && rule.held_keys == alternative_key,
        error: String::new(),
    }
}

/// RUL-14: why a matcher cannot be used, inline under it.
#[must_use]
pub fn matcher_error(matcher: &UrlMatcher) -> Option<String> {
    matcher.compile().err().map(|error| {
        let text = one_line(&error.to_string());
        let mut chars = text.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    })
}

/// A regular expression's parse error spans lines: the pattern again, a
/// caret under the place, then `error: <what>`. The caret only lines up in a
/// fixed-width font, and the entry just above already shows the pattern, so
/// the row keeps the first line's lead and the last line's reason (RUL-14).
fn one_line(text: &str) -> String {
    let (Some(first), Some(last)) = (text.lines().next(), text.lines().last()) else {
        return String::new();
    };
    if first == last {
        return text.to_owned();
    }
    let lead = first
        .trim_end()
        .trim_end_matches("regex parse error:")
        .trim_end()
        .trim_end_matches(':');
    let reason = last.trim().trim_start_matches("error:").trim();
    format!("{lead}: {reason}")
}

/// RUL-19 "Test…": a link the draft's first matcher would see, for the
/// tester.
#[must_use]
pub fn test_url(draft: &Value) -> String {
    let matcher = rule_of(draft)
        .ok()
        .and_then(|rule| rule.url_matchers.into_iter().next());
    let Some(matcher) = matcher else {
        return String::new();
    };
    let pattern = matcher.pattern.trim();
    let plain: String = pattern
        .chars()
        .filter(|c| !matches!(c, '*' | '^' | '$' | '\\'))
        .collect();
    let plain = plain.trim_start_matches('.').trim_start_matches('/');
    if plain.is_empty() {
        String::new()
    } else if plain.contains('/') {
        format!("https://{plain}")
    } else {
        format!("https://{plain}/")
    }
}

#[cfg(test)]
mod tests {
    use wye_core::Modifier;

    use super::*;

    fn shift() -> Modifiers {
        Modifiers::from_slice(&[Modifier::Shift])
    }

    #[test]
    fn pick_31_a_prefill_becomes_a_domain_matcher_and_a_source_app() {
        let prefill =
            Prefill::parse(r#"{"domain": "github.com", "sourceApp": "com.slack.Slack.desktop"}"#);
        let draft = new_draft(&prefill, "rule-1");
        assert_eq!(
            draft["url-matchers"],
            json!([{"kind": "domain", "pattern": "github.com"}])
        );
        assert_eq!(draft["source-apps"], json!(["com.slack.Slack.desktop"]));
        assert_eq!(draft["target"], json!({"default": true}));
        assert!(check(&draft, shift()).valid);
        let empty = new_draft(
            &Prefill::parse(r#"{"domain": null, "sourceApp": null}"#),
            "rule-2",
        );
        assert_eq!(empty["url-matchers"], json!([]));
        assert_eq!(Prefill::parse("garbage"), Prefill::default());
    }

    #[test]
    fn rul_14_a_parse_error_is_one_line_with_its_reason() {
        let text = "invalid regular expression: regex parse error:\n    ^a([b\n       ^\nerror: unclosed character class";
        assert_eq!(
            one_line(text),
            "invalid regular expression: unclosed character class"
        );
        assert_eq!(one_line("empty pattern"), "empty pattern");
        assert_eq!(one_line(""), "");
    }

    #[test]
    fn rul_18_save_needs_a_name_a_condition_and_valid_matchers() {
        let mut draft = new_draft(&Prefill::default(), "rule-1");
        let check_now = |draft: &Value| check(draft, shift());
        assert!(check_now(&draft).name_missing);
        assert!(check_now(&draft).no_condition);
        draft["name"] = json!("Meet");
        draft["url-matchers"] = json!([{"kind": "regex", "pattern": "meet\\.google\\.com/["}]);
        let result = check_now(&draft);
        assert!(!result.valid);
        assert!(
            result.matcher_errors[0].starts_with("Invalid regular expression"),
            "{result:?}"
        );
        assert!(
            !result.matcher_errors[0].contains('\n'),
            "one line under the entry: {result:?}"
        );
        draft["url-matchers"] = json!([{"kind": "domain", "pattern": "meet.google.com"}]);
        assert!(check_now(&draft).valid);
        draft["url-matchers"] = json!([]);
        draft["held-keys"] = json!(["Ctrl"]);
        assert!(check_now(&draft).valid, "held keys alone are a condition");
    }

    #[test]
    fn rul_27_the_alternative_key_wins_over_the_rule() {
        let mut draft = new_draft(&Prefill::default(), "rule-1");
        draft["name"] = json!("x");
        draft["held-keys"] = json!(["Shift"]);
        assert!(check(&draft, shift()).alternative_key_wins);
        draft["held-keys"] = json!(["Shift", "Ctrl"]);
        assert!(!check(&draft, shift()).alternative_key_wins);
    }

    #[test]
    fn broken_drafts_are_reported() {
        let draft = json!({"name": "x", "target": {"app": "not a desktop id"}});
        assert!(!check(&draft, shift()).error.is_empty());
    }

    #[test]
    fn existing_rules_open_complete_and_keep_their_id() {
        let rules: Vec<Rule> = serde_json::from_value(json!([
            {"name": "a", "id": "gh"},
            {"name": "b"}
        ]))
        .expect("rules");
        let first = draft_for(&rules, 0, "rule-x").expect("draft");
        assert_eq!(first["id"], "gh");
        assert_eq!(first["url-matchers"], json!([]));
        assert_eq!(
            draft_for(&rules, 1, "rule-x").expect("draft")["id"],
            "rule-x"
        );
        assert!(draft_for(&rules, 2, "rule-x").is_none());
    }

    #[test]
    fn fresh_ids_are_unused() {
        let rules: Vec<Rule> =
            serde_json::from_value(json!([{"name": "a", "id": "rule-10"}])).expect("rules");
        assert_eq!(fresh_id(&rules, 16), "rule-11");
        assert_eq!(fresh_id(&[], 16), "rule-10");
    }

    #[test]
    fn rul_19_the_tester_starts_from_the_first_matcher() {
        let draft = |kind: &str, pattern: &str| json!({"name": "x", "url-matchers": [{"kind": kind, "pattern": pattern}]});
        assert_eq!(
            test_url(&draft("domain", "github.com")),
            "https://github.com/"
        );
        assert_eq!(
            test_url(&draft("wildcard", "*.atlassian.net/browse/*")),
            "https://atlassian.net/browse/"
        );
        assert_eq!(test_url(&json!({"name": "x"})), "");
    }
}
