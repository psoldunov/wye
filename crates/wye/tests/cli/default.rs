//! `wye default` (DEF-02, DEF-05).

use crate::support::{Desktop, ONE, WYE, is_symlink};

const MIMEAPPS: &str = "config/mimeapps.list";
const STATE: &str = "state/wye/state.toml";

fn with_fake_one_default() -> Desktop {
    let desktop = Desktop::new();
    desktop.write(
        MIMEAPPS,
        &format!(
            "[Default Applications]\nx-scheme-handler/http={ONE}\nx-scheme-handler/https={ONE}\n"
        ),
    );
    desktop
}

#[test]
fn set_status_unset_round_trip() {
    let desktop = with_fake_one_default();
    desktop.install_wye();

    let status = desktop.wye(&["default"]).expect_code(0);
    assert_eq!(status.stdout, "fake-one.desktop is your default browser\n");

    desktop.wye(&["default", "set"]).expect_code(0);
    let mimeapps = desktop.read(MIMEAPPS);
    assert!(
        mimeapps.contains(&format!("x-scheme-handler/https={WYE}")),
        "{mimeapps}"
    );
    assert!(
        mimeapps.contains(&format!("x-scheme-handler/http={WYE}")),
        "{mimeapps}"
    );
    assert_eq!(
        desktop.read(STATE),
        "previous-default-browser = \"fake-one.desktop\"\n"
    );

    let status = desktop.wye(&["default", "status"]).expect_code(0);
    assert_eq!(status.stdout, "Wye is your default browser\n");

    // Setting again keeps the remembered browser rather than remembering Wye.
    desktop.wye(&["default", "set"]).expect_code(0);
    assert!(desktop.read(STATE).contains(ONE));

    let unset = desktop.wye(&["default", "unset"]).expect_code(0);
    assert_eq!(
        unset.stdout,
        "Fake One (fake-one.desktop) is your default browser again\n"
    );
    let mimeapps = desktop.read(MIMEAPPS);
    assert!(
        mimeapps.contains(&format!("x-scheme-handler/https={ONE}")),
        "{mimeapps}"
    );
    assert!(!desktop.read(STATE).contains(ONE));
}

#[test]
fn status_without_a_default() {
    let desktop = Desktop::new();
    let status = desktop.wye(&["default", "status"]).expect_code(0);
    assert_eq!(status.stdout, "No default browser is set\n");
}

#[test]
fn set_refuses_without_wyes_desktop_entry() {
    let desktop = with_fake_one_default();
    let before = desktop.read(MIMEAPPS);
    let run = desktop.wye(&["default", "set"]).expect_code(1);
    assert!(run.stderr.contains("install Wye first"), "{run:#?}");
    assert_eq!(desktop.read(MIMEAPPS), before);
}

#[test]
fn set_reports_a_managed_mimeapps_list() {
    let desktop = Desktop::new();
    desktop.install_wye();
    desktop.write("managed/mimeapps.list", "[Default Applications]\n");
    std::os::unix::fs::symlink(
        desktop.path("managed/mimeapps.list"),
        desktop.path(MIMEAPPS),
    )
    .unwrap();
    let run = desktop.wye(&["default", "set"]).expect_code(1);
    assert!(run.stderr.contains("managed by another tool"), "{run:#?}");
    assert!(run.stderr.contains(WYE), "{run:#?}");
    assert!(is_symlink(&desktop.path(MIMEAPPS)));
    assert_eq!(
        desktop.read("managed/mimeapps.list"),
        "[Default Applications]\n"
    );
}

#[test]
fn unset_needs_a_remembered_browser() {
    let desktop = Desktop::new();
    let run = desktop.wye(&["default", "unset"]).expect_code(1);
    assert!(run.stderr.contains("does not remember"), "{run:#?}");
}

#[test]
fn unset_refuses_a_browser_that_is_gone() {
    let desktop = Desktop::new();
    desktop.write(STATE, "previous-default-browser = \"gone.desktop\"\n");
    let run = desktop.wye(&["default", "unset"]).expect_code(1);
    assert!(run.stderr.contains("no longer installed"), "{run:#?}");
    assert!(desktop.read(STATE).contains("gone.desktop"));
}
