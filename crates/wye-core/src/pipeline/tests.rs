use super::*;

use crate::keys::Modifier;
use crate::matcher::{MatcherKind, UrlMatcher};
use crate::rule::Rule;
use crate::source::SourceAppSpec;
use crate::target::DesktopId;

/// Every target is available except the ones listed.
#[derive(Default)]
struct Apps {
    missing: Vec<Target>,
}

impl Apps {
    fn without(targets: &[&Target]) -> Self {
        Self {
            missing: targets.iter().map(|&t| t.clone()).collect(),
        }
    }
}

impl Availability for Apps {
    fn is_available(&self, target: &Target) -> bool {
        !self.missing.contains(target)
    }
}

fn app(id: &str) -> Target {
    Target::App(DesktopId::new(id).unwrap())
}

fn firefox() -> Target {
    app("firefox.desktop")
}

fn chromium() -> Target {
    app("chromium.desktop")
}

fn spotify() -> Target {
    app("spotify.desktop")
}

fn keys(modifiers: &[Modifier]) -> Modifiers {
    Modifiers::from_slice(modifiers)
}

/// Firefox is the primary browser and Chromium the alternative one.
fn config() -> Config {
    let mut config = Config::default();
    config.browsers.primary = firefox();
    config.browsers.alternative = chromium();
    config
}

fn domain(pattern: &str) -> UrlMatcher {
    UrlMatcher {
        kind: MatcherKind::Domain,
        pattern: pattern.to_owned(),
    }
}

fn rule(name: &str, matcher: UrlMatcher, target: Target) -> Rule {
    Rule {
        id: None,
        name: name.to_owned(),
        enabled: true,
        target,
        url_matchers: vec![matcher],
        source_apps: Vec::new(),
        held_keys: Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform: false,
    }
}

fn pipeline(config: Config) -> Pipeline {
    Pipeline::with_shipped_data(config)
}

fn request(url: &str) -> LinkRequest {
    LinkRequest::new(url, EntryPoint::Handler)
}

fn resolve(config: Config, request: &LinkRequest) -> Resolution {
    resolve_with(config, request, &Apps::default())
}

fn resolve_with(config: Config, request: &LinkRequest, apps: &Apps) -> Resolution {
    pipeline(config).resolve(request, apps).unwrap()
}

fn has_step(resolution: &Resolution, wanted: impl Fn(&Step) -> bool) -> bool {
    resolution.steps.iter().any(wanted)
}

const SPOTIFY_TRACK: &str = "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC";

#[test]
fn no_match_opens_the_primary_browser() {
    let resolution = resolve(config(), &request("https://example.com/"));
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::Fallback { target } if *target == firefox()
    )));
    assert!(!resolution.hold_until_unlock);
}

#[test]
fn unavailable_primary_browser_asks_with_the_picker() {
    let apps = Apps::without(&[&firefox()]);
    let resolution = resolve_with(config(), &request("https://example.com/"), &apps);
    assert_eq!(resolution.target, Target::Picker);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::TargetMissing { target } if *target == firefox()
    )));
}

#[test]
fn alternative_key_opens_the_alternative_browser() {
    let mut request = request("https://example.com/");
    request.held = keys(&[Modifier::Shift]);
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.target, chromium());
    assert_eq!(resolution.decision, Decision::AlternativeKey);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::AlternativeKey { .. }
    )));
}

#[test]
fn alternative_key_beats_a_matching_rule() {
    let mut config = config();
    config.rules.push(rule(
        "Example",
        domain("example.com"),
        app("epiphany.desktop"),
    ));
    let mut request = request("https://example.com/");
    request.held = keys(&[Modifier::Shift]);
    let resolution = resolve(config, &request);
    assert_eq!(resolution.target, chromium());
    assert_eq!(resolution.decision, Decision::AlternativeKey);
}

#[test]
fn alternative_key_needs_the_exact_modifier_set() {
    let mut request = request("https://example.com/");
    request.held = keys(&[Modifier::Ctrl, Modifier::Shift]);
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
}

#[test]
fn forced_alternative_opens_the_alternative_browser() {
    let mut request = request("https://example.com/");
    request.force = Force::Alternative;
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.target, chromium());
    assert_eq!(resolution.decision, Decision::AlternativeKey);
}

#[test]
fn before_rule_beats_a_web_app_mapping() {
    let mut config = config();
    config.apps.insert("spotify".to_owned(), spotify());
    config
        .rules
        .push(rule("Spotify", domain("open.spotify.com"), chromium()));
    let resolution = resolve(config, &request(SPOTIFY_TRACK));
    assert_eq!(resolution.target, chromium());
    assert!(matches!(
        resolution.decision,
        Decision::Rule {
            position: RunPosition::Before,
            ..
        }
    ));
}

#[test]
fn mapping_beats_an_after_rule() {
    let mut config = config();
    config.apps.insert("spotify".to_owned(), spotify());
    let mut after = rule("Spotify", domain("open.spotify.com"), chromium());
    after.run = RunPosition::After;
    config.rules.push(after);
    let resolution = resolve(config, &request(SPOTIFY_TRACK));
    assert_eq!(resolution.target, spotify());
    assert_eq!(
        resolution.decision,
        Decision::Mapping {
            service: "spotify".to_owned()
        }
    );
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::MappingMatched { .. }
    )));
}

#[test]
fn after_rule_applies_when_no_mapping_matches() {
    let mut config = config();
    let mut after = rule("Example", domain("example.com"), chromium());
    after.run = RunPosition::After;
    config.rules.push(after);
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(resolution.target, chromium());
    assert!(matches!(
        resolution.decision,
        Decision::Rule {
            position: RunPosition::After,
            ..
        }
    ));
}

#[test]
fn first_matching_rule_wins() {
    let mut config = config();
    config
        .rules
        .push(rule("First", domain("example.com"), chromium()));
    config.rules.push(rule(
        "Second",
        domain("example.com"),
        app("epiphany.desktop"),
    ));
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(resolution.target, chromium());
    assert_eq!(
        resolution.decision,
        Decision::Rule {
            index: 0,
            name: "First".to_owned(),
            position: RunPosition::Before
        }
    );
}

#[test]
fn mapping_to_default_does_not_match() {
    let mut config = config();
    config.apps.insert("spotify".to_owned(), Target::Default);
    let resolution = resolve(config, &request(SPOTIFY_TRACK));
    assert_eq!(resolution.decision, Decision::Fallback);
    assert_eq!(resolution.target, firefox());
    assert!(!has_step(&resolution, |s| matches!(
        s,
        Step::MappingMatched { .. } | Step::MappingTargetMissing { .. }
    )));
}

#[test]
fn mapping_with_missing_target_is_skipped_and_the_pipeline_continues() {
    let mut config = config();
    config.apps.insert("spotify".to_owned(), spotify());
    let mut after = rule("Spotify", domain("open.spotify.com"), chromium());
    after.run = RunPosition::After;
    config.rules.push(after);
    let apps = Apps::without(&[&spotify()]);
    let resolution = resolve_with(config, &request(SPOTIFY_TRACK), &apps);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::MappingTargetMissing { target, .. } if *target == spotify()
    )));
    assert_eq!(resolution.target, chromium());
    assert!(matches!(resolution.decision, Decision::Rule { .. }));
}

#[test]
fn rule_with_default_target_opens_the_primary_browser() {
    let mut config = config();
    config
        .rules
        .push(rule("Example", domain("example.com"), Target::Default));
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(resolution.target, firefox());
    assert!(matches!(resolution.decision, Decision::Rule { .. }));
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::DefaultIsPrimary { target } if *target == firefox()
    )));
}

// A configuration built in code is sanitised too: Default never comes out.
#[test]
fn default_browsers_in_code_never_resolve_to_default() {
    let mut config = Config::default();
    config.browsers.primary = Target::Default;
    config.browsers.alternative = Target::Default;
    config
        .rules
        .push(rule("Example", domain("example.com"), Target::Default));
    config.apps.insert("unknown-svc".into(), chromium());
    let pipeline = pipeline(config);
    assert_eq!(pipeline.config().browsers.primary, Target::Picker);
    assert!(pipeline.config().apps.is_empty());
    let mut held = request("https://example.com/");
    held.held = keys(&[Modifier::Shift]);
    for request in [
        request("https://example.com/"),
        request("https://other.example/"),
        held,
    ] {
        let resolution = pipeline.resolve(&request, &Apps::default()).unwrap();
        assert_eq!(resolution.target, Target::Picker, "{}", request.url);
    }
}

#[test]
fn rule_with_unavailable_target_asks_with_the_picker() {
    let mut config = config();
    config
        .rules
        .push(rule("Example", domain("example.com"), chromium()));
    let apps = Apps::without(&[&chromium()]);
    let resolution = resolve_with(config, &request("https://example.com/"), &apps);
    assert_eq!(resolution.target, Target::Picker);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::TargetMissing { .. }
    )));
}

#[test]
fn rule_options_reach_the_resolution() {
    let mut config = config();
    let mut example = rule("Example", domain("example.com"), chromium());
    example.open_in_background = true;
    example.force_new_window = true;
    config.rules.push(example);
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(
        resolution.options,
        OpenOptions {
            background: true,
            new_window: true
        }
    );
}

#[test]
fn source_app_rule_matches_only_with_that_source() {
    let mut config = config();
    let mut slack = rule("Slack", domain("example.com"), chromium());
    slack.source_apps = vec![SourceAppSpec::Executable("slack".to_owned())];
    config.rules.push(slack);

    let mut from_slack = request("https://example.com/");
    from_slack.source = SourceApp {
        desktop_id: None,
        executable: Some("slack".to_owned()),
    };
    assert_eq!(resolve(config.clone(), &from_slack).target, chromium());

    let mut from_other = request("https://example.com/");
    from_other.source = SourceApp {
        desktop_id: None,
        executable: Some("thunderbird".to_owned()),
    };
    assert_eq!(resolve(config, &from_other).target, firefox());
}

#[test]
fn unknown_source_never_matches_a_source_app_rule() {
    let mut config = config();
    let mut slack = rule("Slack", domain("example.com"), chromium());
    slack.source_apps = vec![SourceAppSpec::Executable("slack".to_owned())];
    config.rules.push(slack);
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(resolution.decision, Decision::Fallback);
    assert_eq!(resolution.target, firefox());
}

#[test]
fn invalid_rule_is_skipped_and_the_next_rule_matches() {
    let mut config = config();
    let broken = UrlMatcher {
        kind: MatcherKind::Regex,
        pattern: "(unclosed".to_owned(),
    };
    config
        .rules
        .push(rule("Broken", broken, app("epiphany.desktop")));
    config
        .rules
        .push(rule("Good", domain("example.com"), chromium()));
    let resolution = resolve(config, &request("https://example.com/"));
    assert_eq!(resolution.target, chromium());
    assert_eq!(
        resolution.decision,
        Decision::Rule {
            index: 1,
            name: "Good".to_owned(),
            position: RunPosition::Before
        }
    );
}

#[test]
fn extension_links_force_the_picker() {
    let request = LinkRequest::new("https://example.com/", EntryPoint::Extension);
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.target, Target::Picker);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ForcedPicker { by_extension: true }
    )));
}

// ADV-11 and PIPE-06: the bypass key and the alternative key (the escape
// hatch) both beat the extension's forced picker (ADV-10).
#[test]
fn bypass_and_alternative_keys_stop_the_extension_from_forcing_the_picker() {
    for (held, expected) in [(Modifier::Alt, firefox()), (Modifier::Shift, chromium())] {
        let mut request = LinkRequest::new("https://example.com/", EntryPoint::Extension);
        request.held = keys(&[held]);
        let resolution = resolve(config(), &request);
        assert_eq!(resolution.target, expected, "{held:?}");
        assert!(!has_step(&resolution, |s| matches!(
            s,
            Step::ForcedPicker { .. }
        )));
    }
}

#[test]
fn explicit_pick_beats_the_alternative_key() {
    let mut request = LinkRequest::new("https://example.com/", EntryPoint::Extension);
    request.held = keys(&[Modifier::Shift]);
    request.force = Force::Picker;
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.decision, Decision::AlternativeKey);
    assert_eq!(resolution.target, Target::Picker);
}

#[test]
fn extension_does_not_force_the_picker_when_the_setting_is_off() {
    let mut config = config();
    config.advanced.force_picker_from_extension = false;
    let request = LinkRequest::new("https://example.com/", EntryPoint::Extension);
    let resolution = resolve(config, &request);
    assert_eq!(resolution.target, firefox());
}

#[test]
fn forced_picker_opens_the_picker() {
    let mut request = request("https://example.com/");
    request.force = Force::Picker;
    let resolution = resolve(config(), &request);
    assert_eq!(resolution.target, Target::Picker);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ForcedPicker {
            by_extension: false
        }
    )));
}

#[test]
fn locked_screen_skips_the_picker_for_the_alternative_browser() {
    let mut config = config();
    config.browsers.primary = Target::Picker;
    config.picker.skip_when_locked = true;
    let mut request = request("https://example.com/");
    request.screen_locked = true;
    let resolution = resolve(config, &request);
    assert_eq!(resolution.target, chromium());
    assert!(!resolution.hold_until_unlock);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::LockedScreen { .. }
    )));
}

#[test]
fn locked_screen_with_picker_as_alternative_holds_until_unlock() {
    let mut config = config();
    config.browsers.primary = Target::Picker;
    config.browsers.alternative = Target::Picker;
    config.picker.skip_when_locked = true;
    let mut request = request("https://example.com/");
    request.screen_locked = true;
    let resolution = resolve(config, &request);
    assert_eq!(resolution.target, Target::Picker);
    assert!(resolution.hold_until_unlock);
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::HeldUntilUnlock
    )));
}

#[test]
fn locked_screen_keeps_the_picker_when_skipping_is_off() {
    let mut config = config();
    config.browsers.primary = Target::Picker;
    let mut request = request("https://example.com/");
    request.screen_locked = true;
    let resolution = resolve(config, &request);
    assert_eq!(resolution.target, Target::Picker);
    assert!(!resolution.hold_until_unlock);
    assert!(!has_step(&resolution, |s| matches!(
        s,
        Step::LockedScreen { .. }
    )));
}

fn wrapped_github_link() -> String {
    let inner = "https%3A%2F%2Fgithub.com%2Fx%3Futm_source%3Da%26id%3D1";
    format!("https://www.google.com/url?q={inner}")
}

#[test]
fn unwrapping_precedes_cleaning_and_feeds_the_rules() {
    let mut config = config();
    config
        .rules
        .push(rule("GitHub", domain("github.com"), chromium()));
    let resolution = resolve(config, &request(&wrapped_github_link()));
    assert_eq!(resolution.url.as_str(), "https://github.com/x?id=1");
    let unwrapped = resolution
        .steps
        .iter()
        .position(|s| matches!(s, Step::Unwrapped { .. }))
        .expect("unwrapped step");
    let cleaned = resolution
        .steps
        .iter()
        .position(|s| matches!(s, Step::TrackingRemoved { .. }))
        .expect("tracking step");
    assert!(unwrapped < cleaned);
    assert_eq!(resolution.target, chromium());
    assert!(matches!(resolution.decision, Decision::Rule { .. }));
}

#[test]
fn expansion_can_be_turned_off() {
    let mut config = config();
    config.advanced.expand_urls = false;
    let resolution = resolve(config, &request(&wrapped_github_link()));
    assert!(!has_step(&resolution, |s| matches!(
        s,
        Step::Unwrapped { .. }
    )));
    assert_eq!(resolution.url.host_str(), Some("www.google.com"));
}

#[test]
fn enabled_short_link_host_is_reported_as_not_expanded() {
    let resolution = resolve(config(), &request("https://bit.ly/abc"));
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ShortLinkNotExpanded
    )));
    assert_eq!(resolution.url.as_str(), "https://bit.ly/abc");
}

#[test]
fn force_https_upgrades_the_link() {
    let mut config = config();
    config.extras.force_https = true;
    let resolution = resolve(config, &request("http://example.com/a"));
    assert_eq!(resolution.url.as_str(), "https://example.com/a");
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::HttpsForced { .. }
    )));
}

#[test]
fn global_transform_is_reported_as_not_run() {
    let mut config = config();
    config.advanced.transform = true;
    let resolution = resolve(config, &request("https://example.com/"));
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ScriptNotRun(ScriptScope::Global)
    )));
}

// PIPE-05 applies to local HTML files too (DEF-07).
#[test]
fn global_transform_is_reported_for_local_html() {
    let mut config = config();
    config.advanced.transform = true;
    config.general.open_local_html = true;
    let resolution = resolve(config, &request("file:///tmp/a.html"));
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ScriptNotRun(ScriptScope::Global)
    )));
}

#[test]
fn rule_transform_is_reported_as_not_run() {
    let mut config = config();
    let mut example = rule("Example", domain("example.com"), chromium());
    example.transform = true;
    config.rules.push(example);
    let resolution = resolve(config, &request("https://example.com/"));
    assert!(has_step(&resolution, |s| matches!(
        s,
        Step::ScriptNotRun(ScriptScope::Rule)
    )));
    assert!(!has_step(&resolution, |s| matches!(
        s,
        Step::ScriptNotRun(ScriptScope::Global)
    )));
}

#[test]
fn unsupported_scheme_is_rejected() {
    let err = pipeline(config())
        .resolve(&request("mailto:a@b.c"), &Apps::default())
        .unwrap_err();
    assert_eq!(err, Rejected::UnsupportedScheme("mailto".to_owned()));
}

#[test]
fn malformed_link_is_rejected() {
    let err = pipeline(config())
        .resolve(&request("not a url"), &Apps::default())
        .unwrap_err();
    assert!(matches!(err, Rejected::Malformed(_)));
}

#[test]
fn local_html_is_rejected_unless_enabled() {
    let html = request("file:///tmp/a.html");
    let err = pipeline(config())
        .resolve(&html, &Apps::default())
        .unwrap_err();
    assert_eq!(err, Rejected::UnsupportedScheme("file".to_owned()));

    let mut enabled = config();
    enabled.general.open_local_html = true;
    let resolution = resolve(enabled.clone(), &html);
    assert_eq!(resolution.url.as_str(), "file:///tmp/a.html");
    assert_eq!(resolution.target, firefox());

    let text = request("file:///tmp/a.txt");
    let err = pipeline(enabled)
        .resolve(&text, &Apps::default())
        .unwrap_err();
    assert_eq!(err, Rejected::UnsupportedScheme("file".to_owned()));
}

#[test]
fn own_app_receives_the_translated_link() {
    let pipeline = pipeline(config());
    let url = Url::parse(SPOTIFY_TRACK).unwrap();
    assert_eq!(
        pipeline.launch_url(&url, &spotify()),
        "spotify:track:4uLU6hMCjMI75M1A2tKUQC"
    );
}

#[test]
fn other_targets_receive_the_link_unchanged() {
    let pipeline = pipeline(config());
    let url = Url::parse(SPOTIFY_TRACK).unwrap();
    assert_eq!(pipeline.launch_url(&url, &firefox()), SPOTIFY_TRACK);
    assert_eq!(pipeline.launch_url(&url, &Target::Picker), SPOTIFY_TRACK);
}

#[test]
fn every_step_has_a_description() {
    let url = Url::parse("https://example.com/").unwrap();
    let steps = vec![
        Step::Unwrapped {
            wrapper: "google".to_owned(),
            url: url.clone(),
        },
        Step::ShortLinkNotExpanded,
        Step::TrackingRemoved {
            params: vec!["utm_source".to_owned()],
            url: url.clone(),
        },
        Step::HttpsForced { url },
        Step::ScriptNotRun(ScriptScope::Global),
        Step::ScriptNotRun(ScriptScope::Rule),
        Step::AlternativeKey { target: chromium() },
        Step::RuleMatched {
            index: 0,
            name: "Example".to_owned(),
            position: RunPosition::Before,
            target: chromium(),
        },
        Step::MappingMatched {
            service: "Spotify".to_owned(),
            target: spotify(),
        },
        Step::MappingTargetMissing {
            service: "Spotify".to_owned(),
            target: spotify(),
        },
        Step::Fallback { target: firefox() },
        Step::DefaultIsPrimary { target: firefox() },
        Step::TargetMissing { target: firefox() },
        Step::ForcedPicker { by_extension: true },
        Step::ForcedPicker {
            by_extension: false,
        },
        Step::LockedScreen { target: chromium() },
        Step::HeldUntilUnlock,
    ];
    for step in steps {
        assert!(!step.to_string().is_empty(), "{step:?}");
    }
}
