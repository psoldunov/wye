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
    assert_eq!(std::fs::read_dir(parent).expect("dir").count(), 1);
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
    };
    state.save(&path).expect("saved");
    let text = std::fs::read_to_string(&path).expect("read");
    for key in [
        "previous-kdeglobals-browser",
        "onboarding-done",
        "dismissed-callouts",
        "last-settings-page",
        "rules-help-seen",
        "kept-default",
        "script-errors-notified",
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
