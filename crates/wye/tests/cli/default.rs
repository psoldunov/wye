//! `wye default` (DEF-02, DEF-05).

use crate::support::{Desktop, ONE, TWO, WYE, is_symlink};

const MIMEAPPS: &str = "config/mimeapps.list";
const STATE: &str = "state/wye/state.toml";

/// A desktop whose `mimeapps.list` lists `id` as the http and https handler.
fn listing(id: &str) -> Desktop {
    let desktop = Desktop::new();
    desktop.write(
        MIMEAPPS,
        &format!(
            "[Default Applications]\nx-scheme-handler/http={id}\nx-scheme-handler/https={id}\n"
        ),
    );
    desktop
}

/// A desktop where Wye is installed and is the default browser.
fn with_wye_default() -> Desktop {
    let desktop = listing(WYE);
    desktop.install_wye();
    desktop
}

fn with_fake_one_default() -> Desktop {
    listing(ONE)
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

/// An unreadable state file may hold the only record of the previous default
/// (DEF-05): `set` reports it and changes nothing.
#[test]
fn set_refuses_an_unreadable_state_file() {
    let desktop = with_fake_one_default();
    desktop.install_wye();
    desktop.write(STATE, "previous-default-browser = 3\n");
    let mimeapps = desktop.read(MIMEAPPS);
    let run = desktop.wye(&["default", "set"]).expect_code(1);
    assert!(run.stderr.contains("not a valid state file"), "{run:#?}");
    assert_eq!(desktop.read(STATE), "previous-default-browser = 3\n");
    assert_eq!(desktop.read(MIMEAPPS), mimeapps);
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
    let desktop = with_wye_default();
    let run = desktop.wye(&["default", "unset"]).expect_code(1);
    assert!(run.stderr.contains("does not remember"), "{run:#?}");
}

#[test]
fn unset_refuses_a_browser_that_is_gone() {
    let desktop = with_wye_default();
    desktop.write(STATE, "previous-default-browser = \"gone.desktop\"\n");
    let run = desktop.wye(&["default", "unset"]).expect_code(1);
    assert!(run.stderr.contains("no longer installed"), "{run:#?}");
    assert!(desktop.read(STATE).contains("gone.desktop"));
}

#[test]
fn unset_keeps_a_default_the_user_changed_since() {
    // DEF-05: Wye took over from Fake One, then the user chose Fake Two.
    let desktop = Desktop::new();
    desktop.install_wye();
    desktop.write(
        MIMEAPPS,
        &format!(
            "[Default Applications]\nx-scheme-handler/http={TWO}\nx-scheme-handler/https={TWO}\n"
        ),
    );
    desktop.write(STATE, &format!("previous-default-browser = \"{ONE}\"\n"));
    let before = desktop.read(MIMEAPPS);
    let run = desktop.wye(&["default", "unset"]).expect_code(0);
    assert_eq!(
        run.stdout,
        "Wye is not your default browser (fake-two.desktop is); nothing changed\n"
    );
    assert_eq!(desktop.read(MIMEAPPS), before);
    assert!(desktop.read(STATE).contains(ONE));

    let empty = Desktop::new();
    let run = empty.wye(&["default", "unset"]).expect_code(0);
    assert_eq!(
        run.stdout,
        "Wye is not your default browser; nothing changed\n"
    );
}

// Wye is listed as the default, but its entry is not visible to this process
// (a different `XDG_DATA_DIRS`, say): `listing(WYE)` without `install_wye`.

#[test]
fn status_sees_wye_as_default_without_its_entry() {
    let status = listing(WYE).wye(&["default", "status"]).expect_code(0);
    assert_eq!(status.stdout, "Wye is your default browser\n");
}

#[test]
fn unset_restores_the_remembered_browser_without_wyes_entry() {
    let desktop = listing(WYE);
    desktop.write(STATE, &format!("previous-default-browser = \"{ONE}\"\n"));
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
fn set_never_remembers_an_opener() {
    // DEF-06: restoring an entry that runs xdg-open would loop back to Wye.
    let desktop = Desktop::new();
    desktop.install_wye();
    desktop.write(
        "data/applications/opener.desktop",
        "[Desktop Entry]\nType=Application\nName=Opener\nExec=xdg-open %u\n\
         MimeType=x-scheme-handler/http;x-scheme-handler/https;\n",
    );
    desktop.write(
        MIMEAPPS,
        "[Default Applications]\nx-scheme-handler/http=opener.desktop\n\
         x-scheme-handler/https=opener.desktop\n",
    );
    desktop.write(STATE, &format!("previous-default-browser = \"{ONE}\"\n"));
    desktop.wye(&["default", "set"]).expect_code(0);
    assert!(desktop.read(STATE).contains(ONE), "{}", desktop.read(STATE));

    // A remembered opener (from an older Wye) is never restored.
    desktop.write(STATE, "previous-default-browser = \"opener.desktop\"\n");
    let before = desktop.read(MIMEAPPS);
    let run = desktop.wye(&["default", "unset"]).expect_code(1);
    assert!(run.stderr.contains("sends links back"), "{run:#?}");
    assert_eq!(desktop.read(MIMEAPPS), before);
}
