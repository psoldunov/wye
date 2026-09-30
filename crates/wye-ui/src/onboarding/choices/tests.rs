use wye_api::TargetCapabilities;

use super::*;

fn info(kind: TargetKind, target: Value, name: &str, browser: Option<&str>) -> TargetInfo {
    TargetInfo {
        target,
        kind,
        name: name.to_owned(),
        short_name: None,
        icon: Some(name.to_lowercase()),
        badge: None,
        browser: browser.map(str::to_owned),
        capabilities: TargetCapabilities::default(),
        packaging: None,
        missing: false,
    }
}

fn app(id: &str, name: &str) -> TargetInfo {
    info(TargetKind::App, json!({"app": id}), name, None)
}

/// Eight browsers out of order, a profile, a private window, a missing
/// app, and the Picker.
fn inventory() -> TargetInventory {
    let mut gone = app("gone.desktop", "Aardvark");
    gone.missing = true;
    let mut targets = vec![
        info(TargetKind::Picker, picker_target(), "Picker", None),
        app("zen.desktop", "Zen"),
        app("firefox.desktop", "Firefox"),
        app("chromium.desktop", "Chromium"),
        app("brave.desktop", "Brave"),
        app("vivaldi.desktop", "Vivaldi"),
        app("edge.desktop", "Edge"),
        app("librewolf.desktop", "LibreWolf"),
        app("chrome.desktop", "Chrome"),
        gone,
    ];
    targets.push(info(
        TargetKind::Profile,
        json!({"profile": {"app": "chrome.desktop", "id": "Profile 1"}}),
        "Work (Chrome)",
        Some("chrome.desktop"),
    ));
    targets.push(info(
        TargetKind::Private,
        json!({"private": "firefox.desktop"}),
        "Private (Firefox)",
        Some("firefox.desktop"),
    ));
    TargetInventory { targets }
}

fn none() -> BTreeSet<String> {
    BTreeSet::new()
}

fn names(rows: &[ListRow]) -> Vec<&str> {
    rows.iter().map(|row| row.name.as_str()).collect()
}

#[test]
fn the_first_six_browsers_are_checked_without_profiles_or_private_windows() {
    // ONB-03
    let shown = initial_shown(&inventory(), &none());
    let targets: Vec<&Value> = shown.iter().filter_map(entry_target).collect();
    assert_eq!(shown.len(), PRESELECTED);
    // Alphabetical: Brave, Chrome, Chromium, Edge, Firefox, LibreWolf.
    assert_eq!(
        targets,
        [
            &json!({"app": "brave.desktop"}),
            &json!({"app": "chrome.desktop"}),
            &json!({"app": "chromium.desktop"}),
            &json!({"app": "edge.desktop"}),
            &json!({"app": "firefox.desktop"}),
            &json!({"app": "librewolf.desktop"}),
        ]
    );
}

#[test]
fn fewer_than_six_browsers_are_all_checked() {
    let few = TargetInventory {
        targets: vec![app("a.desktop", "A"), app("b.desktop", "B")],
    };
    assert_eq!(initial_shown(&few, &none()).len(), 2);
}

#[test]
fn apps_a_web_service_owns_are_not_browsers() {
    // TGT-05
    let foreign = BTreeSet::from(["zen.desktop".to_owned(), "firefox.desktop".to_owned()]);
    let shown = initial_shown(&inventory(), &foreign);
    assert!(
        shown
            .iter()
            .filter_map(entry_target)
            .all(|target| !foreign.contains(app_id(target).unwrap_or_default()))
    );
    let rows = checklist(&inventory(), &foreign, &[]);
    assert!(!names(&rows).contains(&"Zen"));
}

#[test]
fn the_checklist_lists_checked_rows_first_then_browsers_then_profiles() {
    // ONB-03
    let inventory = inventory();
    let shown = effective_shown(&json!({}), &inventory, &none());
    let rows = checklist(&inventory, &none(), &shown);
    assert_eq!(
        names(&rows),
        [
            "Brave",
            "Chrome",
            "Chromium",
            "Edge",
            "Firefox",
            "LibreWolf",
            // unchecked browsers, then profiles; no missing app, no private window
            "Vivaldi",
            "Zen",
            "Work (Chrome)",
        ]
    );
    assert!(rows[..6].iter().all(|row| row.checked));
    assert!(rows[6..].iter().all(|row| !row.checked));
}

#[test]
fn a_stored_list_wins_over_the_preselection() {
    let inventory = inventory();
    let config =
        json!({"browsers": {"shown": [{"target": {"app": "zen.desktop"}, "hotkey": "z"}]}});
    let shown = effective_shown(&config, &inventory, &none());
    assert_eq!(shown.len(), 1);
    assert_eq!(seed_patch(&config, &inventory, &none()), None);
    assert_eq!(checklist(&inventory, &none(), &shown)[0].name, "Zen");
}

#[test]
fn an_empty_configuration_is_seeded_once() {
    // ONB-03
    let inventory = inventory();
    let patch = seed_patch(&json!({}), &inventory, &none()).expect("patch");
    assert_eq!(
        patch["browsers"]["shown"].as_array().map(Vec::len),
        Some(PRESELECTED)
    );
    assert!(
        seed_patch(&json!({"browsers": {"shown": []}}), &inventory, &none()).is_some(),
        "an empty stored list is no list"
    );
    assert_eq!(
        seed_patch(&json!({}), &TargetInventory::default(), &none()),
        None
    );
}

#[test]
fn toggling_checks_at_the_end_and_unchecks_in_place_keeping_hotkeys() {
    let firefox = json!({"app": "firefox.desktop"});
    let zen = json!({"app": "zen.desktop"});
    let shown = vec![json!({"target": firefox, "hotkey": "f"})];
    let with_zen = toggled(&shown, &zen, true);
    assert_eq!(with_zen.len(), 2);
    assert_eq!(with_zen[0]["hotkey"], "f");
    assert_eq!(with_zen[1], json!({"target": zen}));
    assert_eq!(
        toggled(&with_zen, &firefox, false),
        [json!({"target": zen})]
    );
    assert_eq!(toggled(&shown, &firefox, true), shown);
    assert_eq!(toggled(&shown, &zen, false), shown);
}

#[test]
fn the_primary_menu_lists_the_previous_default_first_and_presets_the_picker() {
    // ONB-03
    let previous = AppRef {
        id: "firefox.desktop".to_owned(),
        name: "Firefox".to_owned(),
        icon: None,
    };
    let choices = primary_choices(&inventory(), &none(), Some(&previous), None);
    assert_eq!(choices[0].name, "Firefox");
    assert_eq!(choices[1].name, "Picker");
    assert_eq!(choices.iter().filter(|c| c.name == "Firefox").count(), 1);
    let checked: Vec<&str> = choices
        .iter()
        .filter(|c| c.checked)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(checked, ["Picker"]);
}

#[test]
fn without_a_previous_default_the_picker_leads_and_a_stored_primary_is_checked() {
    let current = json!({"app": "zen.desktop"});
    let choices = primary_choices(&inventory(), &none(), None, Some(&current));
    assert_eq!(choices[0].name, "Picker");
    let checked: Vec<&str> = choices
        .iter()
        .filter(|c| c.checked)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(checked, ["Zen"]);
    // A previous default that is not installed any more is not offered.
    let gone = AppRef {
        id: "gone.desktop".to_owned(),
        name: "Aardvark".to_owned(),
        icon: None,
    };
    let choices = primary_choices(&inventory(), &none(), Some(&gone), None);
    assert_eq!(choices[0].name, "Picker");
}

#[test]
fn choices_become_merge_patches() {
    assert_eq!(
        primary_patch(&json!({"app": "zen.desktop"})),
        json!({"browsers": {"primary": {"app": "zen.desktop"}}})
    );
    assert_eq!(
        launch_patch(false),
        json!({"general": {"launch-at-login": false}})
    );
    assert_eq!(
        shown_patch(&[json!({"target": {"picker": true}})]),
        json!({"browsers": {"shown": [{"target": {"picker": true}}]}})
    );
}
