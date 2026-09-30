//! Profiles end to end: Chromium badges, Firefox classic and group profiles.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use wye_core::target_menu::Badge;

use super::*;

fn chromium_dir(state: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Local State"), state).unwrap();
    dir
}

fn badge_of(profiles: &[Profile], id: &str) -> Option<Badge> {
    profiles.iter().find(|p| p.id == id)?.badge.clone()
}

// DISC-08, DISC-06
#[test]
fn chromium_uses_the_account_picture_named_in_local_state() {
    let dir = chromium_dir(
        r#"{"profile":{"info_cache":{
            "Profile 1":{"name":"Work","gaia_picture_file_name":"Photo.png",
                         "profile_highlight_color":-14671840}
        }}}"#,
    );
    let profile = dir.path().join("Profile 1");
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join("Photo.png"), b"png").unwrap();
    let profiles = read_chromium(dir.path()).unwrap();
    assert_eq!(profiles[0].avatar, Some(profile.join("Photo.png")));
    assert_eq!(
        profiles[0].badge,
        Some(Badge::Image(
            profile.join("Photo.png").to_string_lossy().into_owned()
        ))
    );
}

#[test]
fn chromium_falls_back_to_the_usual_picture_name() {
    let dir = chromium_dir(
        r#"{"profile":{"info_cache":{"Default":{"name":"Me","gaia_picture_file_name":"Missing.png"}}}}"#,
    );
    let profile = dir.path().join("Default");
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join(CHROMIUM_AVATAR), b"png").unwrap();
    let profiles = read_chromium(dir.path()).unwrap();
    assert_eq!(profiles[0].avatar, Some(profile.join(CHROMIUM_AVATAR)));
}

#[test]
fn chromium_does_not_follow_a_picture_name_out_of_the_profile() {
    let dir = chromium_dir(
        r#"{"profile":{"info_cache":{"Default":{"name":"Me","gaia_picture_file_name":"../Local State"}}}}"#,
    );
    fs::create_dir_all(dir.path().join("Default")).unwrap();
    let profiles = read_chromium(dir.path()).unwrap();
    assert_eq!(profiles[0].avatar, None);
}

#[test]
fn chromium_without_a_picture_draws_an_initial_on_its_colour() {
    let dir = chromium_dir(
        r#"{"profile":{"info_cache":{
            "Profile 1":{"name":"work","profile_highlight_color":-14671840,
                         "default_avatar_fill_color":-1},
            "Profile 2":{"name":"Home","default_avatar_fill_color":-16738680},
            "Profile 3":{"name":"Third"}
        }}}"#,
    );
    let profiles = read_chromium(dir.path()).unwrap();
    // The highlight colour beats the avatar fill; -14671840 is 0xFF202020.
    assert_eq!(
        badge_of(&profiles, "Profile 1"),
        Some(Badge::Initial {
            text: "W".into(),
            color: 0x20_20_20
        })
    );
    assert_eq!(
        badge_of(&profiles, "Profile 2"),
        Some(Badge::Initial {
            text: "H".into(),
            color: badge::rgb_from_argb(-16_738_680)
        })
    );
    assert_eq!(
        badge_of(&profiles, "Profile 3"),
        Some(Badge::Initial {
            text: "T".into(),
            color: badge::hashed_color("Third")
        })
    );
}

// DISC-08: classic Firefox profiles get a hashed colour and an initial.
#[test]
fn classic_firefox_profiles_get_a_hashed_colour_and_an_initial() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("profiles.ini"),
        "[Profile0]\nName=default-release\nIsRelative=1\nPath=abcd.default-release\n",
    )
    .unwrap();
    let profiles = read_firefox(dir.path()).unwrap();
    assert_eq!(
        profiles[0].badge,
        Some(Badge::Initial {
            text: "D".into(),
            color: badge::hashed_color("default-release")
        })
    );
}

const GROUP_INI: &str = "[General]\nStartWithLastProfile=1\nStoreID=ab12cd34\n\n\
    [Profile0]\nName=default-release\nIsRelative=1\nPath=abcd.default-release\nDefault=1\n";

fn group_store(dir: &Path, rows: &[&str]) {
    let folder = dir.join("Profile Groups");
    fs::create_dir_all(&folder).unwrap();
    let connection = Connection::open(folder.join("ab12cd34.sqlite")).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE Profiles (id INTEGER PRIMARY KEY, path TEXT NOT NULL, name TEXT NOT NULL, \
             avatar TEXT NOT NULL, themeId TEXT NOT NULL, themeFg TEXT NOT NULL, themeBg TEXT NOT NULL);",
        )
        .unwrap();
    for row in rows {
        connection
            .execute_batch(&format!("INSERT INTO Profiles VALUES ({row});"))
            .unwrap();
    }
}

// DISC-07
#[test]
fn group_profiles_join_the_classic_ones() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("profiles.ini"), GROUP_INI).unwrap();
    fs::create_dir_all(dir.path().join("abcd.default-release")).unwrap();
    fs::create_dir_all(dir.path().join("Profiles/qq.Work")).unwrap();
    group_store(
        dir.path(),
        &[
            "1, 'abcd.default-release', 'Personal', 'flower', 't', '#fff', '#e5297e'",
            "2, 'Profiles/qq.Work', 'Work', 'briefcase', 't', '#fff', '#1a73e8'",
        ],
    );
    let mut warnings = Vec::new();
    let profiles = read_firefox_with_groups(dir.path(), &mut warnings).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let summary: Vec<_> = profiles
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            ("abcd.default-release", "Personal"),
            ("Profiles/qq.Work", "Work")
        ],
        "the profile in both lists appears once, under its classic ID, with the group's name"
    );
    assert_eq!(
        profiles[0].badge,
        Some(Badge::Initial {
            text: "P".into(),
            color: 0xe5_29_7e
        })
    );
    assert_eq!(profiles[1].path, dir.path().join("Profiles/qq.Work"));
    assert!(profiles[1].path.is_absolute() || profiles[1].path.starts_with(dir.path()));
}

// DISC-07: any error leaves the classic profiles and a warning.
#[test]
fn a_broken_group_store_leaves_the_classic_profiles_and_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("profiles.ini"), GROUP_INI).unwrap();
    let folder = dir.path().join("Profile Groups");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("ab12cd34.sqlite"), b"garbage, not sqlite").unwrap();
    let mut warnings = Vec::new();
    let profiles = read_firefox_with_groups(dir.path(), &mut warnings).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "default-release");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("classic Firefox profiles only"),
        "{warnings:?}"
    );
}

#[test]
fn a_named_but_missing_store_is_a_warning_too() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("profiles.ini"), GROUP_INI).unwrap();
    let mut warnings = Vec::new();
    let profiles = read_firefox_with_groups(dir.path(), &mut warnings).unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(warnings[0].contains("missing"), "{warnings:?}");
}

#[test]
fn without_a_store_id_there_is_nothing_to_warn_about() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("profiles.ini"),
        "[Profile0]\nName=a\nIsRelative=1\nPath=a.dir\n",
    )
    .unwrap();
    let mut warnings = Vec::new();
    let profiles = read_firefox_with_groups(dir.path(), &mut warnings).unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(warnings.is_empty());
    assert!(
        read_firefox_with_groups(&dir.path().join("none"), &mut warnings)
            .unwrap()
            .is_empty()
    );
}
