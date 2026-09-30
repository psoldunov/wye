//! The engine plugged into the core pipeline as its `Transformer`
//! (PIPE-05 global script, PIPE-14 rule script, SCR-22 failures).

use std::cell::RefCell;

use wye_core::hooks::{ScriptError, TransformContext};
use wye_core::matcher::{MatcherKind, UrlMatcher};
use wye_core::pipeline::{EntryPoint, LinkRequest, Step};
use wye_core::{
    Availability, Config, DesktopId, Hooks, Modifiers, Pipeline, Rule, RunPosition, ScriptScope,
    Target,
};
use wye_script::{Run, ScriptSource, ScriptTransformer};

struct AllAvailable;

impl Availability for AllAvailable {
    fn is_available(&self, _: &Target) -> bool {
        true
    }
}

/// Scripts from memory; records every run's logs.
#[derive(Default)]
struct Scripts {
    global: Option<&'static str>,
    rule: Option<&'static str>,
    logs: RefCell<Vec<String>>,
}

impl ScriptSource for Scripts {
    fn source(
        &self,
        scope: ScriptScope,
        context: &TransformContext<'_>,
    ) -> Result<Option<String>, ScriptError> {
        Ok(match scope {
            ScriptScope::Global => self.global,
            ScriptScope::Rule => {
                assert_eq!(context.rule.and_then(|rule| rule.id), Some("x-rule"));
                self.rule
            }
        }
        .map(str::to_owned))
    }

    fn ran(&self, _: ScriptScope, _: &TransformContext<'_>, _: &str, run: &Run) {
        self.logs.borrow_mut().extend(run.logs.iter().cloned());
    }
}

fn app(id: &str) -> Target {
    Target::App(DesktopId::new(id).expect("desktop ID"))
}

fn config() -> Config {
    let mut config = Config::default();
    config.browsers.primary = app("firefox.desktop");
    config.advanced.transform = true;
    config.rules = vec![Rule {
        id: Some("x-rule".into()),
        name: "X".into(),
        enabled: true,
        target: app("chromium.desktop"),
        url_matchers: vec![UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: "x.com".into(),
        }],
        source_apps: Vec::new(),
        held_keys: Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform: true,
    }];
    config
}

const TO_X: &str = r#"export default function transform(url) {
  console.log("global", url.hostname);
  if (url.hostname === "twitter.com") url.hostname = "x.com";
  return url;
}"#;

const ADD_REF: &str = r#"export default function transform(url, context) {
  url.searchParams.set("via", context.rule);
  return url;
}"#;

#[test]
fn pipe_05_and_14_run_the_global_then_the_rule_script() {
    let transformer = ScriptTransformer::new(Scripts {
        global: Some(TO_X),
        rule: Some(ADD_REF),
        ..Scripts::default()
    });
    let hooks = Hooks::none().with_transformer(&transformer);
    let pipeline = Pipeline::with_shipped_data(config());
    let request = LinkRequest::new("https://twitter.com/a/status/1", EntryPoint::Handler);
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .expect("routed");
    // The global script ran before the rules, so the x.com rule matched.
    assert_eq!(resolution.url.as_str(), "https://x.com/a/status/1");
    assert_eq!(resolution.target, app("chromium.desktop"));

    let finished = pipeline.finish(&resolution, &request, None, hooks);
    assert_eq!(finished.url.as_str(), "https://x.com/a/status/1?via=X");
    assert_eq!(*transformer.sources().logs.borrow(), ["global twitter.com"]);
}

#[test]
fn scr_22_a_failing_script_leaves_the_link_unchanged() {
    let transformer = ScriptTransformer::new(Scripts {
        global: Some("export default function transform() {\n  nope();\n}"),
        ..Scripts::default()
    });
    let hooks = Hooks::none().with_transformer(&transformer);
    let pipeline = Pipeline::with_shipped_data(config());
    let request = LinkRequest::new("https://example.com/", EntryPoint::Handler);
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .expect("routed");
    assert_eq!(resolution.url.as_str(), "https://example.com/");
    let failed = resolution.steps.iter().find_map(|step| match step {
        Step::ScriptFailed { scope, message } => Some((*scope, message.clone())),
        _ => None,
    });
    let (scope, message) = failed.expect("recorded the failure");
    assert_eq!(scope, ScriptScope::Global);
    assert!(message.starts_with("line 2: ReferenceError"), "{message}");
}

#[test]
fn a_missing_or_empty_script_keeps_the_link() {
    let transformer = ScriptTransformer::new(Scripts {
        global: Some("  \n"),
        ..Scripts::default()
    });
    let hooks = Hooks::none().with_transformer(&transformer);
    let pipeline = Pipeline::with_shipped_data(config());
    let request = LinkRequest::new("https://x.com/", EntryPoint::Handler);
    let resolution = pipeline
        .resolve_with(&request, &AllAvailable, hooks)
        .expect("routed");
    assert!(
        resolution
            .steps
            .contains(&Step::ScriptUnchanged(ScriptScope::Global))
    );
    let finished = pipeline.finish(&resolution, &request, None, hooks);
    assert_eq!(finished.url.as_str(), "https://x.com/");
}
