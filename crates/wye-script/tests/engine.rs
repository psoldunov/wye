//! The engine against the spec's examples and limits
//! (16-script-editor.md: SCR-03, SCR-07, SCR-20 to SCR-23).

use std::time::Duration;

use url::Url;
use wye_script::{Limits, ScriptInput, TEMPLATE, check, run, run_with_limits};

fn url(text: &str) -> Url {
    Url::parse(text).expect("test URL")
}

fn input() -> ScriptInput {
    ScriptInput {
        source_app: Some("com.slack.Slack.desktop".into()),
        entry_point: "handler".into(),
        held_keys: vec!["Ctrl".into()],
        rule: None,
    }
}

/// The run's result as text: the returned link, `kept`, or the error.
fn outcome(source: &str, link: &str) -> String {
    match run(source, &url(link), &input()).result {
        Ok(Some(url)) => url.to_string(),
        Ok(None) => "kept".to_owned(),
        Err(error) => format!("error: {error}"),
    }
}

const REDDIT: &str = r#"
// Open Reddit links on old.reddit.com
export default function transform(url) {
  if (url.hostname.endsWith("reddit.com")) url.hostname = "old.reddit.com";
  return url;
}
"#;

const YOUTUBE: &str = r#"
// Send YouTube links to a self-hosted front end
export default function transform(url) {
  if (url.hostname === "youtu.be") {
    return `https://invidious.example.org/watch?v=${url.pathname.slice(1)}`;
  }
}
"#;

#[test]
fn spec_example_old_reddit() {
    assert_eq!(
        outcome(REDDIT, "https://www.reddit.com/r/rust/"),
        "https://old.reddit.com/r/rust/"
    );
    assert_eq!(
        outcome(REDDIT, "https://example.com/"),
        "https://example.com/"
    );
}

#[test]
fn spec_example_invidious() {
    assert_eq!(
        outcome(YOUTUBE, "https://youtu.be/dQw4w9WgXcQ"),
        "https://invidious.example.org/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(outcome(YOUTUBE, "https://example.com/"), "kept");
}

#[test]
fn scr_03_the_template_keeps_the_link() {
    assert!(check(TEMPLATE).is_ok());
    assert_eq!(
        outcome(TEMPLATE, "https://example.com/a"),
        "https://example.com/a"
    );
}

#[test]
fn scr_20_url_follows_whatwg() {
    let source = r#"
export default function transform(url) {
  url.protocol = "https:";
  url.searchParams.delete("utm_source");
  url.searchParams.append("q", "a b&c");
  url.hash = "top";
  url.port = "8443";
  return url;
}
"#;
    assert_eq!(
        outcome(source, "http://example.com/p?utm_source=x&id=1"),
        "https://example.com:8443/p?id=1&q=a+b%26c#top"
    );
}

#[test]
fn scr_20_url_search_params_standalone() {
    let source = r#"
export default function transform(url) {
  const params = new URLSearchParams("?b=2&a=1&b=3");
  params.set("b", "9");
  params.sort();
  const pairs = [...params].map(([k, v]) => k + v).join(",");
  const fromObject = new URLSearchParams({ x: 1 }).toString();
  const got = params.getAll("b").join("|") + ";" + params.has("a") + ";" + params.size;
  return `https://example.com/?${params}&pairs=${pairs}&obj=${fromObject}&got=${got}`;
}
"#;
    assert_eq!(
        outcome(source, "https://example.com/"),
        "https://example.com/?a=1&b=9&pairs=a1,b9&obj=x=1&got=9;true;2"
    );
}

#[test]
fn scr_20_setting_search_refreshes_search_params() {
    let source = r#"
export default function transform(url) {
  url.search = "?x=1";
  const x = url.searchParams.get("x");
  url.searchParams.set("y", x);
  url.href = "https://other.example/" + url.search;
  return url.searchParams.get("y") === "1" ? url : "https://wrong.example/";
}
"#;
    assert_eq!(
        outcome(source, "https://example.com/?a=b"),
        "https://other.example/?x=1&y=1"
    );
}

#[test]
fn scr_20_relative_urls_and_statics() {
    let source = r#"
export default function transform(url) {
  const next = new URL("../b?c", url);
  const ok = URL.canParse("https://x.y/") && !URL.canParse("nope") && URL.parse("nope") === null;
  return ok ? next : undefined;
}
"#;
    assert_eq!(
        outcome(source, "https://example.com/a/x/y"),
        "https://example.com/a/b?c"
    );
}

#[test]
fn scr_20_invalid_urls_throw_type_errors() {
    let source = r#"
export default function transform() {
  return new URL("not a link");
}
"#;
    let result = outcome(source, "https://example.com/");
    assert!(result.contains("TypeError: Invalid URL"), "{result}");
}

#[test]
fn scr_20_the_context_describes_the_link() {
    let source = r#"
export default function transform(url, context) {
  url.searchParams.set("s", context.sourceApp);
  url.searchParams.set("e", context.entryPoint);
  url.searchParams.set("k", context.heldKeys.join("+"));
  url.searchParams.set("r", String(context.rule));
  return url;
}
"#;
    assert_eq!(
        outcome(source, "https://example.com/"),
        "https://example.com/?s=com.slack.Slack.desktop&e=handler&k=Ctrl&r=null"
    );
}

#[test]
fn scr_20_console_log_is_captured() {
    let source = r#"
export default function transform(url) {
  console.log("host", url.hostname, 1, { a: 1 }, url);
  console.warn(new Error("careful"));
}
"#;
    let run = run(source, &url("https://example.com/"), &input());
    assert_eq!(run.result, Ok(None));
    assert_eq!(
        run.logs,
        [
            "host example.com 1 {\"a\":1} https://example.com/",
            "Error: careful"
        ]
    );
}

#[test]
fn scr_20_nothing_but_the_url_globals() {
    let source = r#"
export default function transform() {
  const missing = ["fetch", "setTimeout", "require", "process", "XMLHttpRequest", "std", "os"]
    .filter((name) => typeof globalThis[name] !== "undefined");
  if (missing.length > 0) throw new Error(missing.join(","));
}
"#;
    assert_eq!(outcome(source, "https://example.com/"), "kept");
}

#[test]
fn scr_20_imports_are_refused() {
    let source = r#"
import fs from "fs";
export default function transform() {}
"#;
    assert!(outcome(source, "https://example.com/").starts_with("error:"));
}

#[test]
fn scr_21_endless_loops_are_stopped() {
    // In the function, and while the module itself is evaluated.
    for source in [
        "export default function transform() {\n  for (;;) {}\n}\n",
        "while (true) {}\nexport default function transform() {}\n",
    ] {
        let run = run(source, &url("https://example.com/"), &input());
        let error = run.result.expect_err("stopped");
        assert!(error.message.contains("longer than 50 ms"), "{error}");
        assert!(run.elapsed < Duration::from_secs(2), "{:?}", run.elapsed);
    }
}

#[test]
fn scr_21_a_memory_bomb_is_stopped() {
    let source = r#"
export default function transform() {
  const hoard = [];
  for (;;) hoard.push("x".repeat(1024 * 1024) + hoard.length);
}
"#;
    let limits = Limits {
        time: Duration::from_secs(10),
        ..Limits::default()
    };
    let error = run_with_limits(source, &url("https://example.com/"), &input(), limits)
        .result
        .expect_err("stopped");
    assert!(error.message.contains("more than 16 MB"), "{error}");
}

#[test]
fn scr_21_deep_recursion_is_an_error() {
    let source = "const f = (n) => f(n + 1) + 1;\nexport default function transform() { f(0); }\n";
    let result = outcome(source, "https://example.com/");
    assert!(result.starts_with("error:"), "{result}");
}

#[test]
fn scr_22_runtime_errors_carry_their_line() {
    let source = "export default function transform(url) {\n  const a = 1;\n  return x.y;\n}\n";
    let error = run(source, &url("https://example.com/"), &input())
        .result
        .expect_err("fails");
    assert_eq!(error.line, Some(3));
    assert!(error.message.contains("ReferenceError"), "{error}");
}

/// A script that throws on its own keeps its error even when the run took
/// longer than the time limit (a slow or busy machine): only a script the
/// limit actually stopped is reported as over it. A zero limit is already
/// past when the script starts, but this script throws before the engine
/// first checks the clock.
#[test]
fn scr_21_scr_22_a_slow_error_is_not_a_time_limit() {
    let source = "export default function transform() {\n  nope();\n}\n";
    let limits = Limits {
        time: Duration::ZERO,
        ..Limits::default()
    };
    let error = run_with_limits(source, &url("https://example.com/"), &input(), limits)
        .result
        .expect_err("fails");
    assert!(error.message.contains("ReferenceError"), "{error}");
    assert_eq!(error.line, Some(2));
}

#[test]
fn scr_22_thrown_values_are_reported() {
    let source = "export default function transform() {\n  throw \"nope\";\n}\n";
    assert_eq!(
        outcome(source, "https://example.com/"),
        "error: Uncaught nope"
    );
}

#[test]
fn scr_23_results_must_be_web_links() {
    let cases = [
        ("return 42;", "type number"),
        ("return {};", "type object"),
        (
            "return \"mailto:a@example.com\";",
            "not an http or https link",
        ),
        ("return \"not a link\";", "not a link"),
        (
            "return new URL(\"ftp://example.com/\");",
            "not an http or https link",
        ),
    ];
    for (body, expected) in cases {
        let source = format!("export default function transform() {{ {body} }}");
        let result = outcome(&source, "https://example.com/");
        assert!(result.contains(expected), "{body}: {result}");
    }
}

#[test]
fn a_script_needs_a_default_function() {
    let result = outcome("export const transform = 1;", "https://example.com/");
    assert!(result.contains("no default export function"), "{result}");
}

#[test]
fn async_functions_are_awaited() {
    let source = "export default async function transform(url) { url.hostname = \"b.example\"; return url; }";
    assert_eq!(outcome(source, "https://a.example/"), "https://b.example/");
}

#[test]
fn scr_23_a_promise_that_never_settles_says_so() {
    let source =
        "export default async function transform(url) { await new Promise(() => {}); return url; }";
    let result = outcome(source, "https://a.example/");
    assert!(result.contains("promise never settled"), "{result}");
}

#[test]
fn scr_07_syntax_errors_have_a_position() {
    let error = check("export default function transform(url) {\n  return url.;\n}\n")
        .expect_err("does not compile");
    assert_eq!(error.line, 2, "{error}");
    assert!(error.column > 0, "{error}");
    assert!(error.message.starts_with("SyntaxError"), "{error}");
    assert!(error.to_string().starts_with("2:"), "{error}");
}

#[test]
fn scr_07_runtime_errors_do_not_fail_the_check() {
    assert!(check("export default function transform() { return x.y; }").is_ok());
}

#[test]
fn scr_07_a_syntax_error_fails_the_run_with_its_line() {
    let error = run(
        "\n\nexport default function (",
        &url("https://example.com/"),
        &input(),
    )
    .result
    .expect_err("fails");
    assert_eq!(error.line, Some(3));
}

#[test]
fn runs_are_isolated() {
    let first = "globalThis.leak = 1; export default function transform() {}";
    let second =
        "export default function transform() { if (globalThis.leak) throw new Error(\"leaked\"); }";
    assert_eq!(outcome(first, "https://example.com/"), "kept");
    assert_eq!(outcome(second, "https://example.com/"), "kept");
}
