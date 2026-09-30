//! PIPE-03 (network expansion), PIPE-05 and PIPE-14 (transform scripts),
//! PIPE-13 (picker choice) with fake hooks.

use std::cell::RefCell;

use super::*;
use crate::hooks::{
    Location, ResolveError, RuleRef, ScriptError, ShortLinkResolver, TransformContext, Transformer,
};
use crate::keys::{Modifier, Modifiers};
use crate::matcher::{MatcherKind, UrlMatcher};
use crate::rule::{Rule, RunPosition};
use crate::target::DesktopId;

struct AllAvailable;

impl Availability for AllAvailable {
    fn is_available(&self, _: &Target) -> bool {
        true
    }
}

fn app(id: &str) -> Target {
    Target::App(DesktopId::new(id).unwrap())
}

fn config() -> Config {
    let mut config = Config::default();
    config.browsers.primary = app("firefox.desktop");
    config.browsers.alternative = app("chromium.desktop");
    config
}

fn rule(name: &str, domain: &str, target: Target, transform: bool) -> Rule {
    Rule {
        id: Some(format!("{name}-id")),
        name: name.to_owned(),
        enabled: true,
        target,
        url_matchers: vec![UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: domain.to_owned(),
        }],
        source_apps: Vec::new(),
        held_keys: Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform,
    }
}

/// Answers every hop from a table of `(url, location)`.
struct Redirects {
    table: Vec<(&'static str, Result<Option<&'static str>, ResolveError>)>,
    asked: RefCell<Vec<String>>,
}

impl Redirects {
    fn new(table: Vec<(&'static str, Result<Option<&'static str>, ResolveError>)>) -> Self {
        Self {
            table,
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl ShortLinkResolver for Redirects {
    fn resolve(&self, url: &Url) -> Result<Option<Location>, ResolveError> {
        self.asked.borrow_mut().push(url.to_string());
        self.table
            .iter()
            .find(|(from, _)| *from == url.as_str())
            .map_or(Ok(None), |(_, answer)| {
                answer.clone().map(|l| l.map(Location::new))
            })
    }
}

/// Replaces a host, records the scopes and contexts it saw, or fails.
#[derive(Default)]
struct Script {
    replace_host: Option<(&'static str, &'static str)>,
    returns: Option<&'static str>,
    fail: Option<ScriptError>,
    seen: RefCell<Vec<Seen>>,
}

/// Scope, source app, rule (`id:name`) and held keys of one call.
type Seen = (
    ScriptScope,
    Option<String>,
    Option<String>,
    Vec<&'static str>,
);

impl Transformer for Script {
    fn transform(
        &self,
        scope: ScriptScope,
        url: &Url,
        context: &TransformContext<'_>,
    ) -> Result<Option<Url>, ScriptError> {
        self.seen.borrow_mut().push((
            scope,
            context.source_app(),
            context
                .rule
                .map(|RuleRef { id, name }| format!("{}:{name}", id.unwrap_or("-"))),
            context.held_keys(),
        ));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        if let Some(text) = self.returns {
            return Ok(Some(Url::parse(text).unwrap()));
        }
        Ok(self.replace_host.and_then(|(from, to)| {
            (url.host_str() == Some(from)).then(|| {
                let mut replaced = url.clone();
                replaced.set_host(Some(to)).unwrap();
                replaced
            })
        }))
    }
}

fn request(url: &str) -> LinkRequest {
    LinkRequest::new(url, EntryPoint::Handler)
}

fn resolve(config: Config, request: &LinkRequest, hooks: Hooks<'_>) -> Resolution {
    Pipeline::with_shipped_data(config)
        .resolve_with(request, &AllAvailable, hooks)
        .unwrap()
}

#[test]
fn resolve_is_resolve_with_no_hooks() {
    let mut config = config();
    config.advanced.transform = true;
    config.rules = vec![rule("R", "example.com", app("firefox.desktop"), true)];
    let pipeline = Pipeline::with_shipped_data(config);
    let request = request("https://bit.ly/abc");
    let plain = pipeline.resolve(&request, &AllAvailable).unwrap();
    let hooked = pipeline
        .resolve_with(&request, &AllAvailable, Hooks::none())
        .unwrap();
    assert_eq!(plain, hooked);
    assert!(plain.steps.contains(&Step::ShortLinkNotExpanded));
    assert!(
        plain
            .steps
            .contains(&Step::ScriptNotRun(ScriptScope::Global))
    );
}

// PIPE-03
#[test]
fn network_expansion_follows_the_chain_and_reports_each_hop() {
    let redirects = Redirects::new(vec![
        ("https://bit.ly/abc", Ok(Some("https://t.co/x"))),
        (
            "https://t.co/x",
            Ok(Some("https://example.com/page?utm_source=a")),
        ),
    ]);
    let hooks = Hooks::none().with_short_links(&redirects);
    let resolution = resolve(config(), &request("https://bit.ly/abc"), hooks);
    // Expansion runs before cleaning (PIPE-03 then PIPE-04).
    assert_eq!(resolution.url.as_str(), "https://example.com/page");
    let hops: Vec<_> = resolution
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::ShortLinkExpanded { url } => Some(url.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        hops,
        ["https://t.co/x", "https://example.com/page?utm_source=a"]
    );
    assert!(!resolution.steps.contains(&Step::ShortLinkNotExpanded));
}

// PIPE-03
#[test]
fn a_short_link_that_lands_on_a_wrapper_is_unwrapped() {
    let redirects = Redirects::new(vec![(
        "https://bit.ly/abc",
        Ok(Some(
            "https://www.google.com/url?q=https%3A%2F%2Fexample.com%2F",
        )),
    )]);
    let hooks = Hooks::none().with_short_links(&redirects);
    let resolution = resolve(config(), &request("https://bit.ly/abc"), hooks);
    assert_eq!(resolution.url.as_str(), "https://example.com/");
    assert!(
        resolution
            .steps
            .iter()
            .any(|s| matches!(s, Step::Unwrapped { wrapper, .. } if wrapper == "google"))
    );
}

// PIPE-03: on failure the pipeline continues with the unexpanded link.
#[test]
fn a_timeout_continues_with_the_link_so_far() {
    let redirects = Redirects::new(vec![("https://bit.ly/abc", Err(ResolveError::Timeout))]);
    let hooks = Hooks::none().with_short_links(&redirects);
    let resolution = resolve(config(), &request("https://bit.ly/abc"), hooks);
    assert_eq!(resolution.url.as_str(), "https://bit.ly/abc");
    assert!(
        resolution
            .steps
            .iter()
            .any(|s| matches!(s, Step::ShortLinkFailed { reason } if reason.contains("in time")))
    );
    assert_eq!(resolution.target, app("firefox.desktop"));
}

// ADV-01 / DLG-EXP-03: nothing is contacted when expansion is off or the
// domain is disabled.
#[test]
fn expansion_off_or_domain_disabled_contacts_nothing() {
    let redirects = Redirects::new(vec![(
        "https://bit.ly/abc",
        Ok(Some("https://example.com/")),
    )]);
    let hooks = Hooks::none().with_short_links(&redirects);

    let mut off = config();
    off.advanced.expand_urls = false;
    resolve(off, &request("https://bit.ly/abc"), hooks);

    let mut disabled = config();
    disabled.advanced.expansion.disabled = vec!["bit.ly".into()];
    resolve(disabled, &request("https://bit.ly/abc"), hooks);

    resolve(config(), &request("https://example.com/"), hooks);
    assert!(redirects.asked.borrow().is_empty());
}

// PIPE-05
#[test]
fn the_global_script_runs_after_cleaning_and_before_rules() {
    let mut config = config();
    config.advanced.transform = true;
    config.rules = vec![rule("New host", "x.com", app("chromium.desktop"), false)];
    let script = Script {
        replace_host: Some(("twitter.com", "x.com")),
        ..Script::default()
    };
    let hooks = Hooks::none().with_transformer(&script);
    let resolution = resolve(
        config,
        &request("https://twitter.com/a/status/1?utm_source=z"),
        hooks,
    );
    assert_eq!(resolution.url.as_str(), "https://x.com/a/status/1");
    assert_eq!(
        resolution.target,
        app("chromium.desktop"),
        "rules match the transformed link"
    );
    let cleaned = resolution
        .steps
        .iter()
        .position(|s| matches!(s, Step::TrackingRemoved { .. }))
        .unwrap();
    let transformed = resolution
        .steps
        .iter()
        .position(|s| {
            matches!(
                s,
                Step::Transformed {
                    scope: ScriptScope::Global,
                    ..
                }
            )
        })
        .unwrap();
    let matched = resolution
        .steps
        .iter()
        .position(|s| matches!(s, Step::RuleMatched { .. }))
        .unwrap();
    assert!(cleaned < transformed && transformed < matched);
}

#[test]
fn the_global_script_is_skipped_when_the_switch_is_off() {
    let script = Script {
        replace_host: Some(("example.com", "example.org")),
        ..Script::default()
    };
    let hooks = Hooks::none().with_transformer(&script);
    let resolution = resolve(config(), &request("https://example.com/"), hooks);
    assert_eq!(resolution.url.host_str(), Some("example.com"));
    assert!(script.seen.borrow().is_empty());
}

// SCR-20 API: the context carries source app, entry point and held keys.
#[test]
fn the_script_context_carries_the_request() {
    let mut config = config();
    config.advanced.transform = true;
    let script = Script::default();
    let mut request = request("https://example.com/");
    request.held = Modifiers::from_slice(&[Modifier::Ctrl]);
    request.source.desktop_id = Some(DesktopId::new("com.slack.Slack").unwrap());
    resolve(config, &request, Hooks::none().with_transformer(&script));
    let seen = script.seen.borrow();
    assert_eq!(
        seen.as_slice(),
        [(
            ScriptScope::Global,
            Some("com.slack.Slack.desktop".to_owned()),
            None,
            vec!["Ctrl"]
        )]
    );
}

#[test]
fn an_unchanged_script_result_is_recorded_as_unchanged() {
    let mut config = config();
    config.advanced.transform = true;
    let script = Script::default();
    let hooks = Hooks::none().with_transformer(&script);
    let resolution = resolve(config.clone(), &request("https://example.com/"), hooks);
    assert!(
        resolution
            .steps
            .contains(&Step::ScriptUnchanged(ScriptScope::Global))
    );

    // Returning an identical link counts as unchanged too.
    let same = Script {
        returns: Some("https://example.com/"),
        ..Script::default()
    };
    let resolution = resolve(
        config,
        &request("https://example.com/"),
        Hooks::none().with_transformer(&same),
    );
    assert!(
        resolution
            .steps
            .contains(&Step::ScriptUnchanged(ScriptScope::Global))
    );
}

// SCR-22
#[test]
fn a_failing_script_leaves_the_link_alone() {
    let mut config = config();
    config.advanced.transform = true;
    let script = Script {
        fail: Some(ScriptError::at_line("x is not defined", 4)),
        ..Script::default()
    };
    let hooks = Hooks::none().with_transformer(&script);
    let resolution = resolve(config, &request("https://example.com/"), hooks);
    assert_eq!(resolution.url.as_str(), "https://example.com/");
    assert!(resolution.steps.iter().any(|s| matches!(
        s,
        Step::ScriptFailed { scope: ScriptScope::Global, message } if message == "line 4: x is not defined"
    )));
    assert_eq!(resolution.target, app("firefox.desktop"));
}

// SCR-23
#[test]
fn a_script_result_that_is_not_web_is_an_error() {
    let mut config = config();
    config.advanced.transform = true;
    for bad in ["javascript:alert(1)", "ftp://example.com/", "mailto:a@b.c"] {
        let script = Script {
            returns: Some(bad),
            ..Script::default()
        };
        let resolution = resolve(
            config.clone(),
            &request("https://example.com/"),
            Hooks::none().with_transformer(&script),
        );
        assert_eq!(resolution.url.as_str(), "https://example.com/", "{bad}");
        assert!(
            resolution
                .steps
                .iter()
                .any(|s| matches!(s, Step::ScriptFailed { message, .. } if message.contains("not an http or https link"))),
            "{bad}"
        );
    }
}

// PIPE-14
#[test]
fn the_rule_script_runs_in_finish_with_the_rule_in_context() {
    let mut config = config();
    config.rules = vec![rule("Reddit", "reddit.com", app("chromium.desktop"), true)];
    let pipeline = Pipeline::with_shipped_data(config);
    let script = Script {
        replace_host: Some(("reddit.com", "old.reddit.com")),
        ..Script::default()
    };
    let hooks = Hooks::none().with_transformer(&script);
    let request = request("https://reddit.com/r/rust");
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .unwrap();
    assert_eq!(
        resolution.url.host_str(),
        Some("reddit.com"),
        "not yet transformed"
    );
    assert!(script.seen.borrow().is_empty());
    assert!(
        !resolution
            .steps
            .contains(&Step::ScriptNotRun(ScriptScope::Rule))
    );

    let finished = pipeline.finish(&resolution, &request, None, hooks);
    assert_eq!(finished.url.as_str(), "https://old.reddit.com/r/rust");
    assert_eq!(finished.target, app("chromium.desktop"));
    assert!(finished.steps.iter().any(|s| matches!(
        s,
        Step::Transformed {
            scope: ScriptScope::Rule,
            ..
        }
    )));
    assert_eq!(
        script.seen.borrow().as_slice(),
        [(
            ScriptScope::Rule,
            None,
            Some("Reddit-id:Reddit".to_owned()),
            Vec::new()
        )]
    );
}

// PIPE-14: only when the rule's Transform URL is on.
#[test]
fn a_rule_without_transform_runs_no_script() {
    let mut config = config();
    config.rules = vec![rule("Plain", "example.com", app("chromium.desktop"), false)];
    let pipeline = Pipeline::with_shipped_data(config);
    let script = Script {
        replace_host: Some(("example.com", "example.org")),
        ..Script::default()
    };
    let hooks = Hooks::none().with_transformer(&script);
    let request = request("https://example.com/");
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .unwrap();
    let finished = pipeline.finish(&resolution, &request, None, hooks);
    assert_eq!(finished.url.host_str(), Some("example.com"));
    assert!(script.seen.borrow().is_empty());
}

#[test]
fn no_rule_script_runs_when_no_rule_decided() {
    let mut config = config();
    config.rules = vec![rule(
        "Plain",
        "other.example",
        app("chromium.desktop"),
        true,
    )];
    let pipeline = Pipeline::with_shipped_data(config);
    let script = Script::default();
    let hooks = Hooks::none().with_transformer(&script);
    let request = request("https://example.com/");
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .unwrap();
    let finished = pipeline.finish(&resolution, &request, None, hooks);
    assert!(script.seen.borrow().is_empty());
    assert_eq!(finished.steps, resolution.steps);
}

// PIPE-14 without a transformer keeps the old trace.
#[test]
fn finish_without_a_transformer_reports_the_rule_script_once() {
    let mut config = config();
    config.rules = vec![rule("Reddit", "reddit.com", app("chromium.desktop"), true)];
    let pipeline = Pipeline::with_shipped_data(config);
    let request = request("https://reddit.com/");
    let resolution = pipeline.resolve(&request, &AllAvailable).unwrap();
    let finished = pipeline.finish(&resolution, &request, None, Hooks::none());
    let count = finished
        .steps
        .iter()
        .filter(|s| **s == Step::ScriptNotRun(ScriptScope::Rule))
        .count();
    assert_eq!(count, 1);
}

// PIPE-13
#[test]
fn finish_applies_the_picker_choice_and_merges_options() {
    let mut config = config();
    config.browsers.primary = Target::Picker;
    let pipeline = Pipeline::with_shipped_data(config);
    let request = request("https://example.com/");
    let resolution = pipeline.resolve(&request, &AllAvailable).unwrap();
    assert_eq!(resolution.target, Target::Picker);

    let private = Target::Private(DesktopId::new("firefox.desktop").unwrap());
    let chosen = Chosen {
        target: private.clone(),
        options: OpenOptions {
            background: true,
            new_window: false,
        },
    };
    let finished = pipeline.finish(&resolution, &request, Some(chosen), Hooks::none());
    assert_eq!(finished.target, private);
    assert!(finished.options.background);
    assert!(!finished.options.new_window);
    assert_eq!(
        finished.steps.last(),
        Some(&Step::PickerChoice { target: private })
    );
}

#[test]
fn finish_keeps_the_rule_options_when_the_user_chooses() {
    let mut config = config();
    let mut picky = rule("Picky", "example.com", Target::Picker, false);
    picky.force_new_window = true;
    config.rules = vec![picky];
    let pipeline = Pipeline::with_shipped_data(config);
    let request = request("https://example.com/");
    let resolution = pipeline.resolve(&request, &AllAvailable).unwrap();
    let chosen = Chosen {
        target: app("chromium.desktop"),
        options: OpenOptions::default(),
    };
    let finished = pipeline.finish(&resolution, &request, Some(chosen), Hooks::none());
    assert!(finished.options.new_window);
}

#[test]
fn step_text_for_the_new_steps() {
    let url = Url::parse("https://example.com/").unwrap();
    assert_eq!(
        Step::ShortLinkExpanded { url: url.clone() }.to_string(),
        "Expanded (short link): https://example.com/"
    );
    assert_eq!(
        Step::ShortLinkFailed {
            reason: "too many redirects".into()
        }
        .to_string(),
        "Short link not fully expanded: too many redirects"
    );
    assert_eq!(
        Step::Transformed {
            scope: ScriptScope::Rule,
            url
        }
        .to_string(),
        "Transformed (rule script): https://example.com/"
    );
    assert_eq!(
        Step::ScriptUnchanged(ScriptScope::Global).to_string(),
        "Transformed (global script): unchanged"
    );
}
