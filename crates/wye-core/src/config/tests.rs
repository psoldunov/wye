use super::*;
use crate::matcher::MatcherKind;
use crate::rule::RunPosition;
use crate::source::SourceAppSpec;
use crate::target::DesktopId;

const SERVICES: &[&str] = &["google-meet", "discord"];

/// The illustrative configuration in 12-data-model.md.
const SPEC_EXAMPLE: &str = r#"
[browsers]
primary = { picker = true }
alternative = { app = "firefox.desktop" }
alternative-key = ["Shift"]

[[browsers.shown]]
target = { app = "app.zen_browser.zen.desktop" }
hotkey = "a"

[[browsers.shown]]
target = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }
hotkey = "c"

[apps]
google-meet = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }

[[rules]]
name = "GitHub in Firefox"
target = { app = "firefox.desktop" }
url-matchers = [{ kind = "domain", pattern = "github.com" }]
source-apps = ["com.slack.Slack.desktop"]
run = "before"

[picker]
icon-size = "large"
hotkeys = "per-target"

[picker.keys]
open = ["Return", "KP_Enter", "space"]
cancel = ["Escape"]
private-modifier = ["Shift"]
"#;

fn id(s: &str) -> DesktopId {
    DesktopId::new(s).unwrap()
}

fn parse(text: &str) -> Loaded {
    Config::parse(text, SERVICES).unwrap()
}

fn messages(loaded: &Loaded) -> Vec<String> {
    loaded.warnings.iter().map(ToString::to_string).collect()
}

#[test]
fn parses_the_spec_example() {
    let loaded = parse(SPEC_EXAMPLE);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let config = loaded.config;
    assert_eq!(
        config.browsers.alternative,
        Target::App(id("firefox.desktop"))
    );
    assert_eq!(config.browsers.shown.len(), 2);
    assert_eq!(config.browsers.shown[1].hotkey.as_deref(), Some("c"));
    assert!(matches!(config.apps["google-meet"], Target::Profile { .. }));
    let rule = &config.rules[0];
    assert_eq!(rule.url_matchers[0].kind, MatcherKind::Domain);
    assert_eq!(
        rule.source_apps,
        [SourceAppSpec::Desktop(id("com.slack.Slack.desktop"))]
    );
    assert_eq!(rule.run, RunPosition::Before);
    assert!(rule.enabled);
    // Keys not in the file keep their defaults.
    assert_eq!(config.picker.keys.next, ["Right", "Tab"]);
    assert!(config.extras.strip_tracking_on_open);
}

#[test]
fn empty_file_is_all_defaults() {
    let loaded = parse("");
    assert!(loaded.warnings.is_empty());
    let config = loaded.config;
    assert_eq!(config, Config::default());
    assert_eq!(config.browsers.primary, Target::Picker);
    assert_eq!(config.browsers.alternative_key.to_string(), "Shift");
    assert_eq!(config.advanced.bypass_key.to_string(), "Alt");
    assert!(config.advanced.expand_urls);
    assert!(config.advanced.force_picker_from_extension);
    assert!(!config.advanced.history);
    assert!(config.general.launch_at_login);
    assert_eq!(config.picker.icon_size, IconSize::Large);
}

#[test]
fn round_trips_through_toml() {
    let config = parse(SPEC_EXAMPLE).config;
    let text = config.to_toml().unwrap();
    let again = parse(&text);
    assert!(again.warnings.is_empty(), "{:?}\n{text}", again.warnings);
    assert_eq!(again.config, config);
}

#[test]
fn reports_unknown_keys() {
    let loaded = parse(
        "[extras]\nforce-http = true\n[nonsense]\na = 1\n\
         [[rules]]\nname = \"r\"\nurl-matchers = [{ pattern = \"a.b\" }]\ncolour = 1\n",
    );
    let unknown = messages(&loaded);
    for path in ["`extras.force-http`", "`nonsense`", "`rules.0.colour`"] {
        assert!(unknown.iter().any(|w| w.contains(path)), "{unknown:?}");
    }
}

#[test]
fn syntax_errors_are_errors() {
    assert!(Config::parse("[browsers\n", SERVICES).is_err());
    assert!(Config::parse("a = \n", SERVICES).is_err());
}

// CFG-01: one value of the wrong type costs only that value.
#[test]
fn type_errors_keep_the_default_and_the_rest_of_the_file() {
    let loaded = parse(
        "[extras]\nforce-https = \"yes\"\nstrip-tracking-on-copy = true\n\
         [browsers]\nalternative-key = [\"Hyper\"]\nalternative = { app = \"a.desktop\" }\n\
         [general]\nlaunch-at-login = false\n",
    );
    let config = &loaded.config;
    assert!(!config.extras.force_https);
    assert!(config.extras.strip_tracking_on_copy);
    assert_eq!(config.browsers.alternative_key.to_string(), "Shift");
    assert_eq!(config.browsers.alternative, Target::App(id("a.desktop")));
    assert!(!config.general.launch_at_login);
    let text = messages(&loaded);
    assert_eq!(text.len(), 2, "{text:?}");
    assert!(text[0].contains("`browsers.alternative-key`"), "{text:?}");
    assert!(text[1].contains("`extras.force-https`"), "{text:?}");
}

#[test]
fn a_section_of_the_wrong_type_keeps_its_defaults() {
    let loaded = parse("general = 5\n[extras]\nforce-https = true\n");
    assert_eq!(loaded.config.general, General::default());
    assert!(loaded.config.extras.force_https);
    assert!(matches!(
        &loaded.warnings[..],
        [ConfigWarning::InvalidValue { key, .. }] if key == "general"
    ));
}

#[test]
fn one_bad_rule_does_not_drop_the_others() {
    let loaded = parse(
        r#"
[[rules]]
name = "first"
url-matchers = [{ pattern = "a.example" }]

[[rules]]
name = "broken"
target = { app = "not a desktop id" }

[[rules]]
url-matchers = [{ pattern = "no-name.example" }]

[[rules]]
name = "last"
url-matchers = [{ kind = "prefix", pattern = "b.example/x" }]
"#,
    );
    let names: Vec<_> = loaded
        .config
        .rules
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, ["first", "last"]);
    let skipped: Vec<_> = loaded
        .warnings
        .iter()
        .map(|w| match w {
            ConfigWarning::InvalidEntry { key, index, .. } => (key.as_str(), *index),
            other => panic!("unexpected warning {other:?}"),
        })
        .collect();
    assert_eq!(skipped, [("rules", 1), ("rules", 2)]);
}

#[test]
fn one_bad_shown_entry_or_mapping_is_skipped_alone() {
    let loaded = parse(
        r#"
[[browsers.shown]]
target = { app = "a.desktop" }

[[browsers.shown]]
target = { browser = "b.desktop" }

[[browsers.shown]]
target = { app = "c.desktop" }

[apps]
discord = { app = "d.desktop" }
google-meet = { app = 5 }
"#,
    );
    let config = &loaded.config;
    let shown: Vec<_> = config
        .browsers
        .shown
        .iter()
        .map(|e| e.target.clone())
        .collect();
    assert_eq!(
        shown,
        [Target::App(id("a.desktop")), Target::App(id("c.desktop"))]
    );
    assert_eq!(config.apps.len(), 1);
    assert_eq!(config.apps["discord"], Target::App(id("d.desktop")));
    let text = messages(&loaded);
    assert_eq!(text.len(), 2, "{text:?}");
    assert!(text[0].contains("`apps.google-meet`"), "{text:?}");
    assert!(text[1].contains("`browsers.shown` entry 2"), "{text:?}");
}

#[test]
fn out_of_range_expansion_values_are_clamped() {
    let loaded = parse("[advanced.expansion]\ntimeout-ms = 70000\nmax-redirects = 300\n");
    // Kept as written; the pipeline clamps.
    assert_eq!(loaded.config.advanced.expansion.timeout_ms, 70000);
    let (fixed, _) = loaded.config.sanitized(SERVICES);
    assert_eq!(fixed.advanced.expansion.timeout_ms, 5000);
    assert_eq!(fixed.advanced.expansion.max_redirects, 10);
    assert!(
        loaded
            .warnings
            .iter()
            .all(|w| matches!(w, ConfigWarning::OutOfRange { .. })),
        "{:?}",
        loaded.warnings
    );
    assert_eq!(loaded.warnings.len(), 2);
}

// Loading must not destroy data on save: the pipeline sanitises instead.
#[test]
fn loading_keeps_values_that_cannot_apply() {
    let text = r#"
[browsers]
primary = { default = true }

[[browsers.shown]]
target = { picker = true }

[apps]
unknown-svc = { app = "a.desktop" }
"#;
    let loaded = parse(text);
    assert_eq!(loaded.warnings.len(), 3, "{:?}", loaded.warnings);
    let saved = loaded.config.to_toml().unwrap();
    let again = parse(&saved).config;
    assert_eq!(again, loaded.config);
    assert_eq!(again.browsers.primary, Target::Default);
    assert_eq!(again.browsers.shown[0].target, Target::Picker);
    assert_eq!(again.apps["unknown-svc"], Target::App(id("a.desktop")));
}

#[test]
fn corrects_values_that_cannot_apply() {
    let text = r#"
[browsers]
primary = { default = true }

[[browsers.shown]]
target = { picker = true }

[[browsers.shown]]
target = { app = "a.desktop" }
hotkey = "x"

[[browsers.shown]]
target = { app = "b.desktop" }
hotkey = "X"

[[browsers.shown]]
target = { app = "c.desktop" }
hotkey = "Escape"

[apps]
discord = { default = true }
myspace = { app = "a.desktop" }

[[rules]]
name = ""

[advanced.expansion]
timeout-ms = 60000
max-redirects = 0
"#;
    let loaded = parse(text);
    let (config, warnings) = loaded.config.sanitized(SERVICES);
    assert_eq!(warnings, loaded.warnings);
    assert_eq!(config.browsers.primary, Target::Picker);
    assert_eq!(config.browsers.shown.len(), 3);
    assert_eq!(config.browsers.shown[0].hotkey.as_deref(), Some("x"));
    assert_eq!(config.browsers.shown[1].hotkey, None);
    assert_eq!(config.browsers.shown[2].hotkey, None);
    assert!(config.apps.is_empty());
    assert_eq!(config.advanced.expansion.timeout_ms, 5000);
    assert_eq!(config.advanced.expansion.max_redirects, 1);
    // The invalid rule stays; the pipeline skips it.
    assert_eq!(config.rules.len(), 1);
    assert_eq!(warnings.len(), 8, "{warnings:#?}");
}

// KEY-11
#[test]
fn hotkeys_with_modifiers_are_ignored() {
    let text = r#"
[[browsers.shown]]
target = { app = "a.desktop" }
hotkey = "Ctrl+a"

[[browsers.shown]]
target = { app = "b.desktop" }
hotkey = "F2"
"#;
    let loaded = parse(text);
    assert_eq!(
        loaded.warnings,
        [ConfigWarning::InvalidHotkey("Ctrl+a".into())]
    );
    let (config, _) = loaded.config.sanitized(SERVICES);
    assert_eq!(config.browsers.shown[0].hotkey, None);
    assert_eq!(config.browsers.shown[1].hotkey.as_deref(), Some("F2"));
}

// KEY-21
#[test]
fn held_modifier_actions_must_differ() {
    let loaded = parse(
        "[picker.keys]\nbackground-modifier = [\"Shift\"]\nnew-window-modifier = [\"Alt\"]\n",
    );
    assert_eq!(
        loaded.warnings,
        [ConfigWarning::ModifierClash {
            first: "picker.keys.private-modifier",
            second: "picker.keys.background-modifier",
        }]
    );
    // Reported only; the values stay.
    let (config, _) = loaded.config.sanitized(SERVICES);
    assert_eq!(config.picker.keys.background_modifier.to_string(), "Shift");
}
