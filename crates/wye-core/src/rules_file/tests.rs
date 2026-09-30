use super::*;
use crate::keys::{Modifier, Modifiers};
use crate::matcher::{MatcherKind, UrlMatcher};
use crate::rule::RunPosition;
use crate::source::SourceAppSpec;
use crate::target::{CustomApp, DesktopId, Target};

fn id(name: &str) -> DesktopId {
    DesktopId::new(name).unwrap()
}

fn simple(name: &str, pattern: &str) -> Rule {
    Rule {
        id: None,
        name: name.to_owned(),
        enabled: true,
        target: Target::App(id("firefox.desktop")),
        url_matchers: vec![UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: pattern.to_owned(),
        }],
        source_apps: Vec::new(),
        held_keys: Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform: false,
    }
}

/// A rule that sets every field.
fn full() -> Rule {
    Rule {
        id: Some("r1".into()),
        name: "Everything".into(),
        enabled: false,
        target: Target::Profile {
            app: id("google-chrome.desktop"),
            id: "Profile 1".into(),
        },
        url_matchers: vec![
            UrlMatcher {
                kind: MatcherKind::Domain,
                pattern: "github.com".into(),
            },
            UrlMatcher {
                kind: MatcherKind::Regex,
                pattern: r"^example\.com/(a|b)/".into(),
            },
            UrlMatcher {
                kind: MatcherKind::Wildcard,
                pattern: "*.example.org/*".into(),
            },
        ],
        source_apps: vec![
            SourceAppSpec::Desktop(id("com.slack.Slack")),
            SourceAppSpec::Executable("thunderbird".into()),
        ],
        held_keys: Modifiers::from_slice(&[Modifier::Ctrl, Modifier::Shift]),
        open_in_background: true,
        force_new_window: true,
        run: RunPosition::After,
        transform: true,
    }
}

fn bundle(rule: Rule, script: Option<&str>) -> BundledRule {
    BundledRule {
        rule,
        script: script.map(str::to_owned),
    }
}

// RUL-02
#[test]
fn an_exported_file_imports_to_the_same_rules() {
    let script =
        "export default function transform(url) {\n  // a \"quoted\" word\n  return url;\n}\n";
    let rules = vec![
        bundle(simple("Plain", "example.com"), None),
        bundle(full(), Some(script)),
        bundle(
            Rule {
                target: Target::Custom(CustomApp::Executable("/opt/tool/bin/tool".into())),
                ..simple("Custom", "tool.example")
            },
            None,
        ),
    ];
    let text = export(&rules).unwrap();
    let imported = import(&text).unwrap();
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    let expected: Vec<_> = rules
        .iter()
        .map(|b| BundledRule {
            rule: Rule {
                id: None,
                ..b.rule.clone()
            },
            script: b.script.clone(),
        })
        .collect();
    assert_eq!(imported.rules, expected);
    assert_eq!(imported.rules[1].script.as_deref(), Some(script));
}

#[test]
fn the_file_starts_with_its_header_and_leaves_out_ids() {
    let text = export(&[bundle(full(), Some("// script"))]).unwrap();
    assert!(
        text.starts_with("format = \"wye-rules\"\nversion = 1\n"),
        "{text}"
    );
    assert!(
        !text.contains("r1"),
        "the rule's local id must not leak:\n{text}"
    );
    assert!(text.contains("[[rule]]"));
    assert!(text.contains("script = \"// script\""));
}

#[test]
fn an_empty_export_imports_to_nothing() {
    let text = export(&[]).unwrap();
    let imported = import(&text).unwrap();
    assert!(imported.rules.is_empty());
    assert!(imported.skipped.is_empty());
}

// RUL-02: hand-written files work too.
#[test]
fn a_hand_written_file_reads_with_defaults() {
    let text = r#"
format = "wye-rules"
version = 1

[[rule]]
name = "GitHub in Firefox"
target = { app = "firefox.desktop" }
url-matchers = [{ kind = "domain", pattern = "github.com" }]
"#;
    let imported = import(text).unwrap();
    assert_eq!(imported.rules.len(), 1);
    let rule = &imported.rules[0].rule;
    assert_eq!(rule.name, "GitHub in Firefox");
    assert!(rule.enabled, "a rule is on unless the file says otherwise");
    assert_eq!(rule.run, RunPosition::Before);
    assert_eq!(imported.rules[0].script, None);
}

#[test]
fn an_id_in_the_file_is_dropped_so_the_import_gets_a_fresh_one() {
    let text = r#"
format = "wye-rules"
version = 1

[[rule]]
id = "someone-elses"
name = "A"
url-matchers = [{ kind = "domain", pattern = "a.example" }]
"#;
    assert_eq!(import(text).unwrap().rules[0].rule.id, None);
}

// RUL-25: a script with the switch off is kept.
#[test]
fn a_script_without_the_transform_switch_is_kept() {
    let text = r#"
format = "wye-rules"
version = 1

[[rule]]
name = "A"
url-matchers = [{ kind = "domain", pattern = "a.example" }]
script = "// later"
"#;
    let imported = import(text).unwrap();
    assert!(!imported.rules[0].rule.transform);
    assert_eq!(imported.rules[0].script.as_deref(), Some("// later"));
}

// RUL-18: bad rules are skipped one by one.
#[test]
fn invalid_and_unreadable_rules_are_skipped_and_the_rest_imported() {
    let text = r#"
format = "wye-rules"
version = 1

[[rule]]
name = "Good"
url-matchers = [{ kind = "domain", pattern = "good.example" }]

[[rule]]
name = ""
url-matchers = [{ kind = "domain", pattern = "noname.example" }]

[[rule]]
name = "No conditions"

[[rule]]
name = "Bad regex"
url-matchers = [{ kind = "regex", pattern = "(" }]

[[rule]]
name = "Wrong type"
enabled = "yes"

[[rule]]
url-matchers = [{ kind = "domain", pattern = "anon.example" }]

[[rule]]
name = "Also good"
held-keys = ["Alt"]
"#;
    let imported = import(text).unwrap();
    let names: Vec<_> = imported
        .rules
        .iter()
        .map(|b| b.rule.name.as_str())
        .collect();
    assert_eq!(names, ["Good", "Also good"]);
    let skipped: Vec<_> = imported
        .skipped
        .iter()
        .map(|s| (s.position, s.name.as_deref()))
        .collect();
    assert_eq!(
        skipped,
        [
            (2, Some("")),
            (3, Some("No conditions")),
            (4, Some("Bad regex")),
            (5, Some("Wrong type")),
            (6, None),
        ]
    );
    assert!(
        matches!(&imported.skipped[0].problem, Problem::Invalid(e) if e.contains(&RuleError::NoName))
    );
    assert!(
        matches!(&imported.skipped[1].problem, Problem::Invalid(e) if e.contains(&RuleError::NoConditions))
    );
    assert!(
        matches!(&imported.skipped[2].problem, Problem::Invalid(e) if matches!(e[0], RuleError::Matcher { index: 1, .. }))
    );
    assert!(matches!(
        &imported.skipped[3].problem,
        Problem::Unreadable(_)
    ));
    assert!(matches!(
        &imported.skipped[4].problem,
        Problem::Unreadable(_)
    ));
}

#[test]
fn files_that_are_not_rules_files_are_refused() {
    assert!(matches!(
        import("this is [not toml"),
        Err(ImportError::NotToml(_))
    ));
    assert!(matches!(import(""), Err(ImportError::NotRulesFile)));
    assert!(matches!(
        import("[general]\nlaunch-at-login = true\n"),
        Err(ImportError::NotRulesFile)
    ));
    assert!(matches!(
        import("format = \"something-else\"\nversion = 1\n"),
        Err(ImportError::NotRulesFile)
    ));
    assert!(matches!(
        import("format = \"wye-rules\"\n"),
        Err(ImportError::NotRulesFile)
    ));
}

#[test]
fn a_newer_version_is_refused_not_half_read() {
    let error =
        import("format = \"wye-rules\"\nversion = 2\n[[rule]]\nname = \"x\"\n").unwrap_err();
    assert!(matches!(error, ImportError::Version { found: 2 }));
    assert!(error.to_string().contains("version 2"));
}

#[test]
fn a_config_file_is_not_mistaken_for_a_rules_file() {
    // The configuration has `[[rules]]`, the rules file `[[rule]]` and a header.
    let config = crate::config::Config {
        rules: vec![simple("A", "a.example")],
        ..crate::config::Config::default()
    };
    let text = config.to_toml().unwrap();
    assert!(matches!(import(&text), Err(ImportError::NotRulesFile)));
}

#[test]
fn imported_rules_can_be_appended_to_a_configuration() {
    let text = export(&[bundle(simple("A", "a.example"), None)]).unwrap();
    let imported = import(&text).unwrap();
    let mut config = crate::config::Config::default();
    config
        .rules
        .extend(imported.rules.into_iter().map(|b| b.rule));
    assert_eq!(config.rules.len(), 1);
    let again = crate::config::Config::parse(&config.to_toml().unwrap(), &[]).unwrap();
    assert!(again.is_lossless());
    assert_eq!(again.config.rules.len(), 1);
}
