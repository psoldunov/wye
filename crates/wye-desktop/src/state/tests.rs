use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use super::*;

fn id(text: &str) -> DesktopId {
    DesktopId::new(text).expect("valid desktop ID")
}

#[test]
fn missing_file_is_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        State::load(&dir.path().join("state.toml")).expect("loads"),
        State::default()
    );
}

#[test]
fn round_trip_creates_directories() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("a/b/state.toml");
    let state = State {
        previous_default_browser: Some(id("firefox.desktop")),
        ..State::default()
    };
    state.save(&path).expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    assert_eq!(text, "previous-default-browser = \"firefox.desktop\"\n");
    assert_eq!(State::load(&path).expect("loads"), state);
    let parent = path.parent().expect("parent");
    let mut names: Vec<_> = std::fs::read_dir(parent)
        .expect("dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["state.toml", "state.toml.lock"]);
    let mode = std::fs::metadata(parent.join("state.toml.lock"))
        .expect("lock file")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// Two writers whose cycles overlap both keep their fields: the second waits
/// for the first's save instead of loading the file before it.
#[test]
fn interleaved_updates_keep_both_fields() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    let (started, hear) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            State::update(&path, |state| {
                started.send(()).expect("signalled");
                std::thread::sleep(Duration::from_millis(100));
                State {
                    previous_default_browser: Some(id("firefox.desktop")),
                    ..state
                }
            })
            .expect("first update");
        });
        let path = &path;
        scope.spawn(move || {
            hear.recv().expect("first writer started");
            State::update(path, |state| State {
                seen_browsers: Some(vec![id("chromium.desktop")]),
                ..state
            })
            .expect("second update");
        });
    });
    let state = State::load(&path).expect("loads");
    assert_eq!(state.previous_default_browser, Some(id("firefox.desktop")));
    assert_eq!(state.seen_browsers, Some(vec![id("chromium.desktop")]));
}

#[test]
fn update_returns_the_changed_state() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    let after = State::update(&path, |state| State {
        onboarding_done: true,
        ..state
    })
    .expect("updated");
    assert!(after.onboarding_done);
    assert_eq!(State::load(&path).expect("loads"), after);
}

/// An unreadable file is reported and left alone: its fields may be the only
/// record of the previous default browser (DEF-05).
#[test]
fn update_refuses_an_invalid_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    std::fs::write(&path, "previous-default-browser = 3\n").expect("written");
    let result = State::update(&path, |state| State {
        onboarding_done: true,
        ..state
    });
    assert!(matches!(result, Err(StateError::Invalid { .. })));
    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"previous-default-browser = 3\n"
    );
}

#[test]
fn an_unchanged_update_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    State::update(&path, |state| state).expect("updated");
    assert!(!path.exists());
}

#[test]
fn save_waits_for_a_held_lock() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    let lock = StateLock::acquire(&path).expect("locked");
    let state = State {
        onboarding_done: true,
        ..State::default()
    };
    std::thread::scope(|scope| {
        let saving = scope.spawn(|| state.save(&path));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!path.exists(), "saved while the lock was held");
        drop(lock);
        saving.join().expect("joined").expect("saved");
    });
    assert_eq!(State::load(&path).expect("loads"), state);
}

#[test]
fn every_field_round_trips() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    let state = State {
        previous_default_browser: Some(id("firefox.desktop")),
        previous_kdeglobals_browser: Some("!firefox".to_owned()),
        onboarding_done: true,
        dismissed_callouts: vec!["apps-please-read".to_owned()],
        last_settings_page: Some("rules".to_owned()),
        rules_help_seen: true,
        kept_default: Some(id("chromium.desktop")),
        script_errors_notified: vec!["abc".to_owned()],
        extension_host_removed: true,
        seen_browsers: Some(vec![id("firefox.desktop"), id("chromium.desktop")]),
    };
    state.save(&path).expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    for key in [
        "extension-host-removed",
        "previous-kdeglobals-browser",
        "onboarding-done",
        "dismissed-callouts",
        "last-settings-page",
        "rules-help-seen",
        "kept-default",
        "script-errors-notified",
        "seen-browsers",
    ] {
        assert!(text.contains(key), "{key} missing from {text}");
    }
    assert_eq!(State::load(&path).expect("loads"), state);
}

#[test]
fn clearing_writes_an_empty_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    State::default().save(&path).expect("saved");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    assert_eq!(State::load(&path).expect("loads"), State::default());
}

#[test]
fn never_writes_through_a_symlink() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join("elsewhere.toml");
    std::fs::write(&target, "kept\n").expect("written");
    let path = dir.path().join("state.toml");
    std::os::unix::fs::symlink(&target, &path).expect("linked");
    assert!(matches!(
        State::default().save(&path),
        Err(StateError::Write { .. })
    ));
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "kept\n");
    let meta = path.symlink_metadata().expect("metadata");
    assert!(meta.file_type().is_symlink());
}

#[test]
fn invalid_file_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    std::fs::write(&path, "previous-default-browser = 3\n").expect("written");
    assert!(matches!(
        State::load(&path),
        Err(StateError::Invalid { .. })
    ));
}

/// The opt-out from the extension host manifests survives a restart and
/// leaves the file empty while it is off (BEXT-04).
#[test]
fn the_extension_host_opt_out_round_trips_bext_04() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    let removed = State {
        extension_host_removed: true,
        ..State::default()
    };
    removed.save(&path).expect("saved");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "extension-host-removed = true\n"
    );
    assert!(State::load(&path).expect("loads").extension_host_removed);
    State::default().save(&path).expect("saved");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
}

#[test]
fn the_state_file_follows_xdg_state_home() {
    let home = Path::new("/home/u");
    assert_eq!(
        path(home, None),
        Path::new("/home/u/.local/state/wye/state.toml")
    );
    assert_eq!(
        path(home, Some(Path::new("/s"))),
        Path::new("/s/wye/state.toml")
    );
    assert_eq!(
        path(home, Some(Path::new("relative"))),
        Path::new("/home/u/.local/state/wye/state.toml")
    );
}

/// Discovery's record of offered browsers (SHOWN-09) keeps "never scanned"
/// (`None`) apart from "scanned, nothing found" (an empty list).
#[test]
fn seen_browsers_tell_never_scanned_from_none_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state.toml");
    std::fs::write(&path, "onboarding-done = true\n").expect("written");
    assert_eq!(State::load(&path).expect("loads").seen_browsers, None);

    let state = State {
        seen_browsers: Some(Vec::new()),
        ..State::default()
    };
    state.save(&path).expect("saved");
    assert_eq!(State::load(&path).expect("loads"), state);

    let seen = State {
        seen_browsers: Some(vec![id("firefox.desktop")]),
        ..State::default()
    };
    seen.save(&path).expect("saved");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "seen-browsers = [\"firefox.desktop\"]\n"
    );
    assert_eq!(State::load(&path).expect("loads"), seen);
}
