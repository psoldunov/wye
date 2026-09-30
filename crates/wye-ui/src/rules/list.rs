//! The Rules page's list (RUL-01, RUL-07): the rules from the configuration
//! and the one-line summary under each name.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use wye_api::apps::AppList;
use wye_core::{Rule, SourceAppSpec};

/// Matchers named in a summary before "and N more".
const SUMMARY_MATCHERS: usize = 3;
/// Separator between the summary's parts.
const DOT: &str = " \u{b7} ";

/// Desktop ID to app name, for "from Slack".
pub type AppNames = HashMap<String, String>;

/// One row of the list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    /// Position in the configuration.
    pub index: usize,
    pub name: String,
    /// "github.com, gitlab.com · from Slack · Shift".
    pub summary: String,
    pub enabled: bool,
    /// The target in its configuration shape, for `targetLabel`.
    pub target: Value,
}

/// The rules in `GetConfig`'s JSON; none when it has none or does not
/// parse.
#[must_use]
pub fn rules_of(config_json: &str) -> Vec<Rule> {
    serde_json::from_str::<Value>(config_json)
        .ok()
        .and_then(|config| config.get("rules").cloned())
        .and_then(|rules| serde_json::from_value(rules).ok())
        .unwrap_or_default()
}

/// Names by desktop ID from `GetApps(true)`'s JSON (or the fixture's).
#[must_use]
pub fn app_names(apps_json: &str) -> AppNames {
    wye_api::json::decode::<AppList>("AppList", apps_json)
        .map(|list| {
            list.apps
                .into_iter()
                .map(|app| (app.id, app.name))
                .collect()
        })
        .unwrap_or_default()
}

/// The list's rows.
#[must_use]
pub fn rows(rules: &[Rule], names: &AppNames) -> Vec<RowView> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| RowView {
            index,
            name: rule.name.clone(),
            summary: summary(rule, names),
            enabled: rule.enabled,
            target: serde_json::to_value(&rule.target).unwrap_or(Value::Null),
        })
        .collect()
}

/// RUL-07: the conditions in one dimmed line.
#[must_use]
pub fn summary(rule: &Rule, names: &AppNames) -> String {
    let patterns: Vec<&str> = rule
        .url_matchers
        .iter()
        .map(|matcher| matcher.pattern.as_str())
        .collect();
    let matchers = match patterns.len() {
        0 => None,
        count if count <= SUMMARY_MATCHERS => Some(patterns.join(", ")),
        count => Some(format!(
            "{} and {} more",
            patterns
                .get(..SUMMARY_MATCHERS)
                .unwrap_or_default()
                .join(", "),
            count - SUMMARY_MATCHERS
        )),
    };
    let sources = (!rule.source_apps.is_empty()).then(|| {
        let apps: Vec<String> = rule
            .source_apps
            .iter()
            .map(|spec| app_name(spec, names))
            .collect();
        format!("from {}", apps.join(", "))
    });
    let keys = (!rule.held_keys.is_empty()).then(|| {
        rule.held_keys
            .iter()
            .map(wye_core::Modifier::name)
            .collect::<Vec<_>>()
            .join("+")
    });
    let parts: Vec<String> = [matchers, sources, keys].into_iter().flatten().collect();
    if parts.is_empty() {
        "No conditions".to_owned()
    } else {
        parts.join(DOT)
    }
}

/// A source-app row of the rule editor (RUL-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRow {
    /// The value stored in the rule.
    pub spec: String,
    pub name: String,
    /// `Kirigami.Icon.source`, empty for none.
    pub icon: String,
}

/// The rows for `specs` with names and icons from `apps_json`.
#[must_use]
pub fn source_rows(specs: &[String], apps_json: &str) -> Vec<SourceRow> {
    let apps = wye_api::json::decode::<AppList>("AppList", apps_json)
        .map(|list| list.apps)
        .unwrap_or_default();
    let names: AppNames = apps
        .iter()
        .map(|app| (app.id.clone(), app.name.clone()))
        .collect();
    specs
        .iter()
        .map(|spec| {
            let parsed = SourceAppSpec::from(spec.clone());
            let icon = apps
                .iter()
                .find(|app| app.id == *spec)
                .map(|app| crate::settings::icon::source(app.icon.as_deref()))
                .unwrap_or_default();
            SourceRow {
                spec: spec.clone(),
                name: app_name(&parsed, &names),
                icon,
            }
        })
        .collect()
}

/// An app's name, else its desktop ID without `.desktop`, else the
/// executable.
#[must_use]
pub fn app_name(spec: &SourceAppSpec, names: &AppNames) -> String {
    match spec {
        SourceAppSpec::Desktop(id) => names.get(id.as_str()).cloned().unwrap_or_else(|| {
            id.as_str()
                .strip_suffix(".desktop")
                .unwrap_or(id.as_str())
                .to_owned()
        }),
        SourceAppSpec::Executable(name) => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn rule(value: &Value) -> Rule {
        serde_json::from_value(value.clone()).expect("a rule")
    }

    fn names() -> AppNames {
        AppNames::from([("com.slack.Slack.desktop".to_owned(), "Slack".to_owned())])
    }

    #[test]
    fn rul_07_summaries_list_the_conditions() {
        let rule = rule(&json!({
            "name": "Code",
            "url-matchers": [{"pattern": "github.com"}, {"kind": "prefix", "pattern": "gitlab.com"}],
            "source-apps": ["com.slack.Slack.desktop", "discord"],
            "held-keys": ["Shift"]
        }));
        assert_eq!(
            summary(&rule, &names()),
            "github.com, gitlab.com \u{b7} from Slack, discord \u{b7} Shift"
        );
    }

    #[test]
    fn long_matcher_lists_are_shortened() {
        let rule = rule(&json!({
            "name": "Many",
            "url-matchers": [{"pattern": "a.b"}, {"pattern": "c.d"}, {"pattern": "e.f"}, {"pattern": "g.h"}, {"pattern": "i.j"}]
        }));
        assert_eq!(summary(&rule, &names()), "a.b, c.d, e.f and 2 more");
        assert_eq!(
            summary(&self::rule(&json!({"name": "x"})), &names()),
            "No conditions"
        );
    }

    #[test]
    fn rul_16_source_rows_have_names() {
        let apps = json!({"apps": [{"id": "com.slack.Slack.desktop", "name": "Slack"}], "recentSources": []}).to_string();
        let rows = source_rows(&["com.slack.Slack.desktop".into(), "discord".into()], &apps);
        assert_eq!(rows[0].name, "Slack");
        assert_eq!(rows[1].name, "discord");
        assert_eq!(rows[1].icon, "");
    }

    #[test]
    fn unknown_apps_show_their_id() {
        let rule = rule(&json!({"name": "x", "source-apps": ["org.kde.dolphin.desktop"]}));
        assert_eq!(summary(&rule, &AppNames::new()), "from org.kde.dolphin");
    }

    #[test]
    fn rows_come_from_the_configuration() {
        let config = json!({"rules": [
            {"name": "A", "enabled": false, "target": {"app": "firefox.desktop"}, "url-matchers": [{"pattern": "a.b"}]},
            {"name": "B"}
        ]})
        .to_string();
        let rows = rows(&rules_of(&config), &names());
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].enabled);
        assert_eq!(rows[0].target, json!({"app": "firefox.desktop"}));
        assert_eq!(rows[1].index, 1);
        assert!(rules_of("nope").is_empty());
        assert!(app_names("nope").is_empty());
    }
}
