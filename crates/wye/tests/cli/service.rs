//! `wye service` and `wye open` through the service (DEF-04, IN-07).
//! Skips without `dbus-daemon` and `dbus-send`.

use std::process::{Command, Stdio};

use crate::private_bus::{PrivateBus, missing_tools};
use crate::support::{Desktop, ONE};

const PRIMARY_ONE: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";

#[test]
fn open_starts_the_service_through_d_bus_activation() {
    if missing_tools() {
        return;
    }
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let bus = PrivateBus::with_wye_activatable(&desktop).expect("dbus-daemon");
    assert!(!bus.service_running());

    let run = desktop
        .wye_on_bus(&["open", "https://example.com/?utm_source=x"], &bus.address)
        .expect_code(0);
    assert_eq!(run.stderr, "");
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");
    assert!(
        bus.service_running(),
        "the link went through the activated service"
    );
}

#[test]
fn a_rejected_link_through_the_service_exits_2() {
    if missing_tools() {
        return;
    }
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let bus = PrivateBus::with_wye_activatable(&desktop).expect("dbus-daemon");
    let run = desktop
        .wye_on_bus(&["open", "ftp://example.com/"], &bus.address)
        .expect_code(2);
    assert!(run.stderr.contains("ftp"), "{run:#?}");
    assert!(desktop.never_launched(ONE));
}

#[test]
fn open_falls_back_in_process_when_no_bus_answers() {
    // `wye` always runs with a session bus address that points nowhere
    // unless a test gives it one.
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let nowhere = format!("unix:path={}", desktop.path("nothing-here").display());
    let run = desktop
        .wye_on_bus(&["open", "https://example.com/"], &nowhere)
        .expect_code(0);
    assert_eq!(run.stderr, "", "the fallback is silent");
    assert_eq!(desktop.wait_for_log(ONE), "https://example.com/\n");
}

#[test]
fn a_second_service_exits_75() {
    if missing_tools() {
        return;
    }
    let desktop = Desktop::new();
    let bus = PrivateBus::plain(&desktop).expect("dbus-daemon");
    let mut first = Command::new(env!("CARGO_BIN_EXE_wye"))
        .arg("service")
        .env_clear()
        .envs(desktop.env())
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(bus.wait_for_service(), "the first service took the name");

    let second = desktop
        .wye_on_bus(&["service"], &bus.address)
        .expect_code(75);
    assert!(second.stderr.contains("already"), "{second:#?}");

    first.kill().unwrap();
    first.wait().unwrap();
}

#[test]
fn service_activate_starts_the_service_and_exits() {
    if missing_tools() {
        return;
    }
    let desktop = Desktop::new();
    let bus = PrivateBus::with_wye_activatable(&desktop).expect("dbus-daemon");
    desktop
        .wye_on_bus(&["service", "--activate"], &bus.address)
        .expect_code(0);
    assert!(
        bus.wait_for_service(),
        "GEN-01: the autostart entry starts it"
    );
}

#[test]
fn service_only_commands_report_an_unreachable_service() {
    let desktop = Desktop::new();
    for args in [&["menu"][..], &["settings", "rules"], &["clipboard"]] {
        let run = desktop.wye(args).expect_code(1);
        assert!(run.stderr.contains("not reachable"), "{args:?}: {run:#?}");
    }
}
