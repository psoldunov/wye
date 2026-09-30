//! `wye open` (IN-01, IN-07).

use crate::support::{Desktop, ONE, TWO};

const PRIMARY_ONE: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";

#[test]
fn cleans_the_link_and_opens_the_primary_browser() {
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let run = desktop
        .wye(&["open", "https://github.com/x?utm_source=a&id=1"])
        .expect_code(0);
    assert_eq!(run.stderr, "");
    assert_eq!(desktop.wait_for_log(ONE), "https://github.com/x?id=1\n");
    assert!(desktop.never_launched(TWO));
}

#[test]
fn a_rule_picks_the_browser() {
    let desktop = Desktop::new();
    desktop.config(&format!(
        "{PRIMARY_ONE}\n[[rules]]\nname = \"GitHub\"\ntarget = {{ app = \"{TWO}\" }}\n\
         url-matchers = [{{ kind = \"domain\", pattern = \"github.com\" }}]\n"
    ));
    desktop
        .wye(&["open", "https://github.com/x"])
        .expect_code(0);
    assert_eq!(desktop.wait_for_log(TWO), "https://github.com/x\n");
    assert!(desktop.never_launched(ONE));
}

#[test]
fn alternative_opens_the_alternative_browser() {
    let desktop = Desktop::new();
    desktop.config(&format!(
        "{PRIMARY_ONE}alternative = {{ app = \"{TWO}\" }}\n"
    ));
    desktop
        .wye(&["open", "--alternative", "https://example.com/"])
        .expect_code(0);
    assert_eq!(desktop.wait_for_log(TWO), "https://example.com/\n");
}

#[test]
fn pick_conflicts_with_alternative() {
    let desktop = Desktop::new();
    desktop
        .wye(&["open", "--pick", "--alternative", "https://example.com/"])
        .expect_code(2);
}

#[test]
fn the_picker_falls_back_to_a_browser_for_now() {
    let desktop = Desktop::new();
    let run = desktop
        .wye(&["open", "https://example.com/"])
        .expect_code(0);
    assert!(
        run.stderr
            .contains("the picker is not available yet; opening in Fake One"),
        "{run:#?}"
    );
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");
}

#[test]
fn the_fallback_prefers_the_previous_default() {
    let desktop = Desktop::new();
    desktop.write(
        "state/wye/state.toml",
        "previous-default-browser = \"fake-two.desktop\"\n",
    );
    let run = desktop
        .wye(&["open", "https://example.com/"])
        .expect_code(0);
    assert!(run.stderr.contains("opening in Fake Two"), "{run:#?}");
    assert_eq!(desktop.wait_for_log(TWO), "https://example.com/\n");
}

#[test]
fn a_broken_config_still_opens_links() {
    let desktop = Desktop::new();
    desktop.config("[browsers\n");
    let run = desktop
        .wye(&["open", "https://example.com/"])
        .expect_code(0);
    assert!(run.stderr.contains("using defaults"), "{run:#?}");
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");
}

#[test]
fn a_broken_stderr_never_stops_a_link() {
    // A broken config and the picker stand-in both write to stderr, which
    // fails on /dev/full; the link still opens and the exit code is clean.
    let desktop = Desktop::new();
    desktop.config("[browsers\n");
    desktop.write("state/wye/state.toml", "previous-default-browser = 3\n");
    desktop
        .wye_with_broken_stderr(&["open", "https://example.com/"])
        .expect_code(0);
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");

    desktop.wye_with_broken_stderr(&["open"]).expect_code(0);
    desktop
        .wye_with_broken_stderr(&["open", "mailto:x"])
        .expect_code(2);
}

#[test]
fn config_warnings_stay_quiet() {
    let desktop = Desktop::new();
    desktop.config(&format!("unknown-key = 1\n{PRIMARY_ONE}"));
    let run = desktop
        .wye(&["open", "https://example.com/"])
        .expect_code(0);
    assert_eq!(run.stderr, "");
    desktop.wait_for_log(ONE);
}

#[test]
fn unsupported_links_are_rejected() {
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let run = desktop.wye(&["open", "mailto:x"]).expect_code(2);
    assert!(
        run.stderr.contains("can't open this kind of link"),
        "{run:#?}"
    );
    assert!(desktop.never_launched(ONE));
}

#[test]
fn other_links_open_when_one_is_rejected() {
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    desktop
        .wye(&["open", "mailto:x", "https://example.com/"])
        .expect_code(2);
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");
}

#[test]
fn no_link_is_not_an_error() {
    let desktop = Desktop::new();
    let run = desktop.wye(&["open"]).expect_code(0);
    assert_eq!(run.stderr, "wye: no link given\n");
}

#[test]
fn a_missing_browser_is_a_launch_error() {
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    std::fs::remove_file(desktop.path("bin/fake-one")).unwrap();
    let run = desktop
        .wye(&["open", "https://example.com/"])
        .expect_code(1);
    assert!(run.stderr.starts_with("wye: "), "{run:#?}");
}
