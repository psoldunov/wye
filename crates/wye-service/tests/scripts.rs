//! Transform scripts on a private bus: `GetScript`, `SetScript`,
//! `RunScript` and `ScriptFileChanged` (SCR-03, SCR-04, SCR-07, SCR-08,
//! SCR-20 to SCR-23). Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;
use std::time::Duration;

use futures_lite::StreamExt as _;
use support::Service;
use wye_api::Error;
use wye_api::scripts::ScriptRun;
use zbus::zvariant::Value;

const GLOBAL_FILE: &str = "config/wye/transform.js";
const RULE_FILE: &str = "config/wye/rules/gh.js";

const TO_X: &str = "export default function transform(url) {\n  if (url.hostname === \"twitter.com\") url.hostname = \"x.com\";\n  return url;\n}\n";

/// How long to wait for a signal that should not come.
const QUIET: Duration = Duration::from_millis(600);
/// How long to wait for a signal that should come.
const PATIENCE: Duration = Duration::from_secs(5);

async fn run(
    service: &Service,
    source: &str,
    url: &str,
    context: HashMap<&str, Value<'_>>,
) -> ScriptRun {
    let text = service
        .wye()
        .await
        .run_script(source, url, context)
        .await
        .expect("RunScript");
    wye_api::json::decode("ScriptRun", &text).expect("ScriptRun JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_03_a_missing_script_answers_the_template() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    for scope in ["global", "rule:gh"] {
        // SCR-09: no file yet.
        assert!(!wye.script_exists(scope).await.expect("ScriptExists"));
        let text = wye.get_script(scope).await.expect("GetScript");
        assert!(
            text.contains("export default function transform(url, context)"),
            "{text}"
        );
    }
    assert!(
        !service.desktop.path(GLOBAL_FILE).exists(),
        "reading wrote a file"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_08_scripts_are_files_next_to_the_configuration() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    wye.set_script("global", TO_X).await.expect("saved");
    wye.set_script("rule:gh", "export default () => {};\n")
        .await
        .expect("saved");
    assert_eq!(service.desktop.read(GLOBAL_FILE), TO_X);
    assert_eq!(
        service.desktop.read(RULE_FILE),
        "export default () => {};\n"
    );
    assert_eq!(wye.get_script("global").await.expect("read"), TO_X);
    assert!(wye.script_exists("global").await.expect("ScriptExists"));
    wye.set_script("global", "  \n").await.expect("saved");
    assert!(!wye.script_exists("global").await.expect("ScriptExists"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_07_a_syntax_error_is_refused_with_its_position() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let refused = wye
        .set_script(
            "global",
            "export default function transform(url) {\n  return url.;\n}\n",
        )
        .await;
    let Err(Error::ScriptSyntax(message)) = refused else {
        panic!("expected ScriptSyntax, got {refused:?}");
    };
    assert!(message.starts_with("2:"), "{message}");
    assert!(!service.desktop.path(GLOBAL_FILE).exists());

    // Runtime errors do not block saving.
    wye.set_script(
        "global",
        "export default function transform() { return x.y; }",
    )
    .await
    .expect("saved");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_scopes_are_invalid_args() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    for scope in ["rules", "rule:", "rule:../../evil", "rule:a/b"] {
        let answer = wye.get_script(scope).await;
        assert!(
            matches!(answer, Err(Error::InvalidArgs(_))),
            "{scope}: {answer:?}"
        );
        let answer = wye.set_script(scope, "").await;
        assert!(
            matches!(answer, Err(Error::InvalidArgs(_))),
            "{scope}: {answer:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_04_run_script_reports_the_result() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let context = HashMap::from([("source-desktop-id", Value::from("com.slack.Slack"))]);
    let changed = run(&service, TO_X, "https://twitter.com/a/status/1", context).await;
    assert!(changed.ok, "{changed:?}");
    assert_eq!(changed.url.as_deref(), Some("https://x.com/a/status/1"));
    assert_eq!(changed.changed, [[8, 9]]);

    let logging = "export default function transform(url, context) {\n  console.log(context.sourceApp, context.rule);\n}\n";
    let context = HashMap::from([
        ("source-desktop-id", Value::from("com.slack.Slack")),
        ("rule", Value::from("GitHub")),
    ]);
    let kept = run(&service, logging, "https://example.com/", context).await;
    assert!(kept.ok);
    assert_eq!(kept.url, None);
    assert_eq!(kept.logs, ["com.slack.Slack.desktop GitHub"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_21_to_23_failures_are_answers_not_errors() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let cases = [
        (
            "export default function transform() {\n  for (;;) {}\n}",
            "longer than 50 ms",
        ),
        (
            "export default function transform() {\n  return missing;\n}",
            "ReferenceError",
        ),
        (
            "export default function transform() { return \"mailto:a@b.c\"; }",
            "not an http or https link",
        ),
    ];
    for (source, expected) in cases {
        let failed = run(&service, source, "https://example.com/", HashMap::new()).await;
        assert!(!failed.ok, "{source}");
        let error = failed.error.unwrap_or_default();
        assert!(error.contains(expected), "{source}: {error}");
    }
    let failed = run(&service, cases[1].0, "https://example.com/", HashMap::new()).await;
    assert_eq!(failed.line, Some(2));

    let invalid = service
        .wye()
        .await
        .run_script(TO_X, "not a link", HashMap::new())
        .await;
    assert!(matches!(invalid, Err(Error::InvalidArgs(_))), "{invalid:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_08_external_edits_are_announced_and_own_saves_are_not() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let mut changes = wye.receive_script_file_changed().await.expect("subscribed");
    // Opening the editor starts the watcher.
    wye.get_script("global").await.expect("read");
    wye.set_script("global", TO_X).await.expect("saved");
    let own = tokio::time::timeout(QUIET, changes.next()).await;
    assert!(own.is_err(), "own save announced: {own:?}");

    service
        .desktop
        .replace(GLOBAL_FILE, "export default function transform() {}\n");
    let signal = tokio::time::timeout(PATIENCE, changes.next())
        .await
        .expect("announced")
        .expect("stream open");
    assert_eq!(signal.args().expect("args").scope, "global");

    service
        .desktop
        .write(RULE_FILE, "export default function transform() {}\n");
    let signal = tokio::time::timeout(PATIENCE, changes.next())
        .await
        .expect("announced")
        .expect("stream open");
    assert_eq!(signal.args().expect("args").scope, "rule:gh");
}

// Scripts on the link path (PIPE-05, PIPE-14, SCR-22).

const SCRIPTED: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n\n[advanced]\ntransform = true\n\n[[rules]]\nid = \"to-two\"\nname = \"X in Two\"\ntarget = { app = \"fake-two.desktop\" }\nurl-matchers = [{ kind = \"domain\", pattern = \"x.com\" }]\ntransform = true\n";

const VIA_RULE: &str = "export default function transform(url, context) {\n  url.searchParams.set(\"via\", context.rule);\n  return url;\n}\n";

fn cli() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([("entry", Value::from("cli"))])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_05_and_14_links_run_the_global_then_the_rule_script() {
    let Some(service) = Service::start(SCRIPTED).await else {
        return;
    };
    service.desktop.write(GLOBAL_FILE, TO_X);
    service
        .desktop
        .write("config/wye/rules/to-two.js", VIA_RULE);
    service
        .wye()
        .await
        .open_link("https://twitter.com/a/status/1", cli())
        .await
        .expect("opened");
    assert_eq!(
        service.launched(),
        [vec![
            "fake-two".to_owned(),
            "https://x.com/a/status/1?via=X+in+Two".to_owned()
        ]]
    );

    // TestLink shows the same scripts in its trace (IN-08).
    let trace = service
        .wye()
        .await
        .test_link("https://twitter.com/b", HashMap::new())
        .await
        .expect("traced");
    assert!(trace.contains("https://x.com/b?via=X+in+Two"), "{trace}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scr_22_a_failing_script_opens_the_link_and_notifies_once() {
    let Some(service) = Service::start(SCRIPTED).await else {
        return;
    };
    service.desktop.write(
        GLOBAL_FILE,
        "export default function transform() {\n  nope();\n}\n",
    );
    let wye = service.wye().await;
    // A trial run never notifies.
    wye.test_link("https://example.com/", HashMap::new())
        .await
        .expect("traced");
    for _ in 0..2 {
        wye.open_link("https://example.com/", cli())
            .await
            .expect("opened");
    }
    assert_eq!(service.launched().len(), 2);
    assert!(
        service
            .launched()
            .iter()
            .all(|argv| argv[1] == "https://example.com/")
    );
    support::eventually("one notification", || async {
        service
            .fakes
            .notifier
            .shown()
            .iter()
            .any(|shown| shown.summary == "Transform script failed")
    })
    .await;
    tokio::time::sleep(QUIET).await;
    let failures: Vec<_> = service
        .fakes
        .notifier
        .shown()
        .into_iter()
        .filter(|shown| shown.summary == "Transform script failed")
        .collect();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].body.contains("line 2"), "{:?}", failures[0]);
}
