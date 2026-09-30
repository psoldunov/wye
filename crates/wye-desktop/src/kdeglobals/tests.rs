use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::PathBuf;

use super::*;
use crate::test_support::Fixture;

fn wye() -> DesktopId {
    DesktopId::new("dev.soldunov.wye.desktop").unwrap()
}

fn kde() -> (Fixture, XdgDirs) {
    let fx = Fixture::new();
    let mut xdg = fx.xdg.clone();
    xdg.current_desktops = vec!["KDE".into()];
    (fx, xdg)
}

fn read(xdg: &XdgDirs) -> String {
    fs::read_to_string(path(xdg)).unwrap_or_default()
}

const SAMPLE: &str = "[General]\nColorScheme=BreezeDark\nBrowserApplication=firefox.desktop\nfont=Noto Sans,10\n\n[KDE]\nwidgetStyle=Breeze\n";

// DEF-02
#[test]
fn sets_the_browser_and_returns_the_previous_value() {
    let (fx, xdg) = kde();
    fx.write("home/.config/kdeglobals", SAMPLE);
    assert_eq!(browser(&xdg).as_deref(), Some("firefox.desktop"));
    let applied = set_browser(&xdg, &wye()).unwrap();
    assert_eq!(
        applied,
        Applied::Set {
            previous: Some("firefox.desktop".into())
        }
    );
    assert_eq!(
        read(&xdg),
        SAMPLE.replace("firefox.desktop", "dev.soldunov.wye.desktop"),
        "only the value changed"
    );
    assert_eq!(browser(&xdg).as_deref(), Some("dev.soldunov.wye.desktop"));
}

#[test]
fn creates_the_file_and_group_when_missing() {
    let (_fx, xdg) = kde();
    assert_eq!(
        set_browser(&xdg, &wye()).unwrap(),
        Applied::Set { previous: None }
    );
    assert_eq!(
        read(&xdg),
        "[General]\nBrowserApplication=dev.soldunov.wye.desktop\n"
    );
}

#[test]
fn adds_the_key_to_an_existing_group_without_the_key() {
    let (fx, xdg) = kde();
    fx.write(
        "home/.config/kdeglobals",
        "[General]\nfont=Noto\n\n[KDE]\nx=1\n",
    );
    assert_eq!(
        set_browser(&xdg, &wye()).unwrap(),
        Applied::Set { previous: None }
    );
    assert_eq!(
        read(&xdg),
        "[General]\nfont=Noto\nBrowserApplication=dev.soldunov.wye.desktop\n\n[KDE]\nx=1\n"
    );
}

#[test]
fn keeps_the_expand_flag_of_an_existing_key() {
    let (fx, xdg) = kde();
    fx.write(
        "home/.config/kdeglobals",
        "[General]\nBrowserApplication[$e]=!/usr/bin/firefox\n",
    );
    assert_eq!(browser(&xdg).as_deref(), Some("!/usr/bin/firefox"));
    assert_eq!(
        set_browser(&xdg, &wye()).unwrap(),
        Applied::Set {
            previous: Some("!/usr/bin/firefox".into())
        }
    );
    assert_eq!(
        read(&xdg),
        "[General]\nBrowserApplication[$e]=dev.soldunov.wye.desktop\n"
    );
}

#[test]
fn setting_twice_is_a_no_op_that_keeps_the_remembered_previous_value() {
    let (fx, xdg) = kde();
    fx.write("home/.config/kdeglobals", SAMPLE);
    set_browser(&xdg, &wye()).unwrap();
    let after_first = read(&xdg);
    assert_eq!(set_browser(&xdg, &wye()).unwrap(), Applied::AlreadySet);
    assert_eq!(read(&xdg), after_first);
}

#[test]
fn only_kde_is_touched() {
    let fx = Fixture::new();
    assert_eq!(fx.xdg.current_desktops, vec!["GNOME"]);
    fx.write("home/.config/kdeglobals", SAMPLE);
    assert_eq!(set_browser(&fx.xdg, &wye()).unwrap(), Applied::NotKde);
    assert_eq!(read(&fx.xdg), SAMPLE);
    assert_eq!(
        restore_browser(&fx.xdg, &wye(), Some("firefox.desktop")).unwrap(),
        Restored::NotKde
    );
    let mut mixed = fx.xdg.clone();
    mixed.current_desktops = vec!["ubuntu".into(), "kde".into()];
    assert!(is_kde(&mixed), "the desktop name is matched ignoring case");
    assert!(!is_kde(&fx.xdg));
}

// DEF-05
#[test]
fn restores_the_previous_value() {
    let (fx, xdg) = kde();
    fx.write("home/.config/kdeglobals", SAMPLE);
    let Applied::Set { previous } = set_browser(&xdg, &wye()).unwrap() else {
        panic!("expected the browser to be set");
    };
    assert_eq!(
        restore_browser(&xdg, &wye(), previous.as_deref()).unwrap(),
        Restored::Restored
    );
    assert_eq!(read(&xdg), SAMPLE, "byte for byte as before");
}

#[test]
fn restoring_without_a_previous_value_removes_the_key() {
    let (fx, xdg) = kde();
    fx.write(
        "home/.config/kdeglobals",
        "[General]\nfont=Noto\n\n[KDE]\nx=1\n",
    );
    set_browser(&xdg, &wye()).unwrap();
    assert_eq!(
        restore_browser(&xdg, &wye(), None).unwrap(),
        Restored::Restored
    );
    assert_eq!(read(&xdg), "[General]\nfont=Noto\n\n[KDE]\nx=1\n");
    assert_eq!(browser(&xdg), None);
}

#[test]
fn restoring_leaves_a_browser_the_user_chose_since() {
    let (fx, xdg) = kde();
    fx.write("home/.config/kdeglobals", SAMPLE);
    set_browser(&xdg, &wye()).unwrap();
    fx.write(
        "home/.config/kdeglobals",
        &SAMPLE.replace("firefox.desktop", "chromium.desktop"),
    );
    assert_eq!(
        restore_browser(&xdg, &wye(), Some("firefox.desktop")).unwrap(),
        Restored::NotOurs
    );
    assert_eq!(browser(&xdg).as_deref(), Some("chromium.desktop"));

    let (_fx2, empty) = kde();
    assert_eq!(
        restore_browser(&empty, &wye(), None).unwrap(),
        Restored::NotOurs,
        "no file, nothing to restore"
    );
}

#[test]
fn other_keys_and_groups_survive_a_round_trip() {
    let (fx, xdg) = kde();
    let original = "# comment\n[General]\nBrowserApplication=a.desktop\nBrowserApplication=b.desktop\n[Other]\nBrowserApplication=keep-me.desktop\n";
    fx.write("home/.config/kdeglobals", original);
    set_browser(&xdg, &wye()).unwrap();
    assert!(read(&xdg).contains("[Other]\nBrowserApplication=keep-me.desktop\n"));
    restore_browser(&xdg, &wye(), Some("a.desktop")).unwrap();
    assert!(read(&xdg).contains("[Other]\nBrowserApplication=keep-me.desktop\n"));
    assert!(read(&xdg).starts_with("# comment\n[General]\nBrowserApplication=a.desktop\n"));
}

#[test]
fn a_managed_file_is_not_replaced() {
    let (fx, xdg) = kde();
    let target = fx.write("store/kdeglobals", SAMPLE);
    fs::create_dir_all(path(&xdg).parent().unwrap()).unwrap();
    symlink(&target, path(&xdg)).unwrap();
    assert!(matches!(
        set_browser(&xdg, &wye()),
        Err(DefaultBrowserError::Managed { .. })
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), SAMPLE);
    assert!(
        fs::symlink_metadata(path(&xdg))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_managed_file_that_already_names_wye_needs_no_write_to_set_but_cannot_be_restored() {
    let (fx, xdg) = kde();
    let target = fx.write(
        "store/kdeglobals",
        "[General]\nBrowserApplication=dev.soldunov.wye.desktop\n",
    );
    fs::create_dir_all(path(&xdg).parent().unwrap()).unwrap();
    symlink(&target, path(&xdg)).unwrap();
    // Setting sees the value is already Wye's and writes nothing.
    assert_eq!(set_browser(&xdg, &wye()).unwrap(), Applied::AlreadySet);
    assert!(matches!(
        restore_browser(&xdg, &wye(), None),
        Err(DefaultBrowserError::Managed { .. })
    ));
}

/// A `kwriteconfig6` that records its arguments, one call per line.
fn recording_kwriteconfig(fx: &Fixture) -> PathBuf {
    let log = fx.path("kwriteconfig.log");
    let script = fx.write(
        "bin/kwriteconfig6",
        &format!("#!/bin/sh\necho \"$*\" >> '{}'\n", log.display()),
    );
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    log
}

// DEF-02, DEF-05: with kwriteconfig6 on PATH, KConfig's own writer (and
// lock) makes the change; Wye does not rewrite the file itself.
#[test]
fn kwriteconfig6_sets_and_restores_the_browser_when_available() {
    let (fx, xdg) = kde();
    let log = recording_kwriteconfig(&fx);
    fx.write("home/.config/kdeglobals", SAMPLE);
    let file = path(&xdg);
    assert_eq!(
        set_browser(&xdg, &wye()).unwrap(),
        Applied::Set {
            previous: Some("firefox.desktop".into())
        }
    );
    assert_eq!(read(&xdg), SAMPLE, "left to kwriteconfig6");
    fx.write(
        "home/.config/kdeglobals",
        "[General]\nBrowserApplication=dev.soldunov.wye.desktop\n",
    );
    assert_eq!(
        restore_browser(&xdg, &wye(), None).unwrap(),
        Restored::Restored
    );
    assert_eq!(
        fs::read_to_string(log).unwrap(),
        format!(
            "--file {0} --group General --key BrowserApplication -- dev.soldunov.wye.desktop\n\
             --file {0} --group General --key BrowserApplication --delete\n",
            file.display()
        )
    );
}
