//! `wye config`.

use crate::support::Desktop;

#[test]
fn path_is_the_default_action() {
    let desktop = Desktop::new();
    let expected = format!("{}\n", desktop.config_path().display());
    assert_eq!(desktop.wye(&["config"]).expect_code(0).stdout, expected);
    assert_eq!(
        desktop.wye(&["config", "path"]).expect_code(0).stdout,
        expected
    );
}

#[test]
fn check_without_a_file() {
    let desktop = Desktop::new();
    let run = desktop.wye(&["config", "check"]).expect_code(0);
    assert!(
        run.stdout.ends_with(": not found, defaults apply\n"),
        "{run:#?}"
    );
}

#[test]
fn check_a_clean_file() {
    let desktop = Desktop::new();
    desktop.config("[browsers]\nprimary = { app = \"fake-one.desktop\" }\n");
    let run = desktop.wye(&["config", "check"]).expect_code(0);
    assert_eq!(
        run.stdout,
        format!("{}: OK\n", desktop.config_path().display())
    );
}

#[test]
fn check_reports_warnings() {
    let desktop = Desktop::new();
    desktop.config("unknown-key = 1\n");
    let run = desktop.wye(&["config", "check"]).expect_code(1);
    assert!(run.stdout.contains("unknown-key"), "{run:#?}");
}

#[test]
fn check_reports_parse_errors() {
    let desktop = Desktop::new();
    desktop.config("[browsers\n");
    let run = desktop.wye(&["config", "check"]).expect_code(2);
    assert!(run.stdout.contains("invalid configuration"), "{run:#?}");
}
