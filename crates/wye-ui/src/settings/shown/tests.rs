use serde_json::json;

use super::*;

fn info(target: &Value, kind: TargetKind, name: &str) -> TargetInfo {
    let text = json!({"target": target, "kind": kind.as_str(), "name": name, "icon": "x"});
    serde_json::from_value(text).expect("target info")
}

fn app(id: &str, name: &str) -> TargetInfo {
    info(&json!({"app": id}), TargetKind::App, name)
}

fn inventory() -> TargetInventory {
    let mut profile = info(
        &json!({"profile": {"app": "chrome.desktop", "id": "P1"}}),
        TargetKind::Profile,
        "Work (Chrome)",
    );
    profile.browser = Some("chrome.desktop".into());
    let mut private = info(
        &json!({"private": "firefox.desktop"}),
        TargetKind::Private,
        "Firefox (Private)",
    );
    private.browser = Some("firefox.desktop".into());
    TargetInventory {
        targets: vec![
            info(&json!({"picker": true}), TargetKind::Picker, "Picker"),
            app("firefox.desktop", "Firefox"),
            app("chrome.desktop", "Chrome"),
            app("discord.desktop", "Discord"),
            private,
            profile,
            info(&json!({"custom": "/opt/tool"}), TargetKind::Custom, "tool"),
        ],
    }
}

fn entry(id: &str, hotkey: Option<&str>) -> Entry {
    Entry {
        target: json!({"app": id}),
        hotkey: hotkey.map(str::to_owned),
    }
}

fn foreign() -> BTreeSet<String> {
    BTreeSet::from(["discord.desktop".to_owned()])
}

fn names(rows: &[Row]) -> Vec<(&str, bool)> {
    rows.iter()
        .map(|row| (row.name.as_str(), row.checked))
        .collect()
}

#[test]
fn checked_rows_come_first_in_the_users_order_then_candidates() {
    // SHOWN-02, SHOWN-03
    let shown = [entry("firefox.desktop", Some("a"))];
    let rows = rows(&inventory(), &shown, &foreign());
    assert_eq!(
        names(&rows),
        [
            ("Firefox", true),
            ("Chrome", false),
            ("Work (Chrome)", false),
            ("tool", false),
            ("Firefox (Private)", false),
        ]
    );
}

#[test]
fn a_service_app_and_the_picker_are_not_candidates() {
    // SHOWN-02: browsers, profiles, added apps, private windows only
    let rows = rows(&inventory(), &[], &foreign());
    assert!(
        rows.iter()
            .all(|row| row.name != "Discord" && row.name != "Picker")
    );
}

#[test]
fn only_an_added_app_can_be_removed() {
    // SHOWN-08
    let rows = rows(&inventory(), &[], &foreign());
    let removable: Vec<_> = rows
        .iter()
        .filter(|row| row.removable)
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(removable, ["tool"]);
}

#[test]
fn a_configured_target_whose_app_is_gone_is_still_listed_as_missing() {
    // APP-10
    let mut inventory = inventory();
    let mut gone = app("gone.desktop", "gone.desktop");
    gone.missing = true;
    inventory.targets.push(gone);
    let rows = rows(&inventory, &[entry("gone.desktop", None)], &foreign());
    assert!(rows[0].missing && rows[0].checked);
}

#[test]
fn an_empty_list_means_every_installed_browser_alphabetically() {
    // the picker is never empty
    let list = effective(&inventory(), &[], &foreign());
    let ids: Vec<_> = list
        .iter()
        .map(|e| e.target["app"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(ids, ["chrome.desktop", "firefox.desktop"]);
    let chosen = [entry("firefox.desktop", None)];
    assert_eq!(effective(&inventory(), &chosen, &foreign()), chosen);
}

#[test]
fn checking_appends_and_unchecking_removes() {
    // SHOWN-03
    let start = [entry("firefox.desktop", Some("f"))];
    let checked = toggle(&start, &json!({"app": "chrome.desktop"}), true);
    assert_eq!(checked.len(), 2);
    assert_eq!(checked[1].target, json!({"app": "chrome.desktop"}));
    let unchecked = toggle(&checked, &json!({"app": "firefox.desktop"}), false);
    assert_eq!(unchecked, [entry("chrome.desktop", None)]);
    assert_eq!(
        toggle(&start, &json!({"app": "firefox.desktop"}), true),
        start
    );
}

#[test]
fn dragging_reorders_checked_rows() {
    // SHOWN-03
    let start = [
        entry("a.desktop", None),
        entry("b.desktop", None),
        entry("c.desktop", None),
    ];
    let moved = move_entry(&start, 2, 0);
    let ids: Vec<_> = moved
        .iter()
        .map(|e| e.target["app"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(ids, ["c.desktop", "a.desktop", "b.desktop"]);
    assert_eq!(move_entry(&start, 0, 9), start);
    assert_eq!(move_entry(&start, 1, 1), start);
}

#[test]
fn a_hotkey_is_unique() {
    // SHOWN-04: choosing one in use clears it on the other row
    let start = [entry("a.desktop", Some("f")), entry("b.desktop", None)];
    let next = set_hotkey(&start, &json!({"app": "b.desktop"}), Some("f"));
    assert_eq!(
        next,
        [entry("a.desktop", None), entry("b.desktop", Some("f"))]
    );
}

#[test]
fn a_hotkey_can_be_cleared() {
    // SHOWN-04: "None"
    let start = [entry("a.desktop", Some("f"))];
    assert_eq!(
        set_hotkey(&start, &json!({"app": "a.desktop"}), None),
        [entry("a.desktop", None)]
    );
}

#[test]
fn a_hotkey_on_an_unchecked_row_checks_it() {
    let next = set_hotkey(&[], &json!({"app": "a.desktop"}), Some("a"));
    assert_eq!(next, [entry("a.desktop", Some("a"))]);
}

#[test]
fn adding_and_removing_an_app() {
    // SHOWN-05, SHOWN-08
    let custom = json!({"custom": "/opt/tool"});
    let added = add(&[], &custom);
    assert_eq!(added.len(), 1);
    assert!(remove(&added, &custom).is_empty());
}

#[test]
fn the_patch_sends_the_whole_list_with_optional_hotkeys() {
    let patch = to_patch(&[entry("a.desktop", Some("a")), entry("b.desktop", None)]);
    assert_eq!(
        patch,
        json!({"browsers": {"shown": [
            {"target": {"app": "a.desktop"}, "hotkey": "a"},
            {"target": {"app": "b.desktop"}}
        ]}})
    );
    let config: wye_core::Config =
        serde_json::from_value(wye_core::merge_patch::apply(&json!({}), &patch)).expect("config");
    assert_eq!(config.browsers.shown.len(), 2);
}

#[test]
fn entries_are_read_from_the_configuration() {
    let config = json!({"browsers": {"shown": [{"target": {"app": "a.desktop"}, "hotkey": "a"}, {"nonsense": 1}]}});
    assert_eq!(entries(&config), [entry("a.desktop", Some("a"))]);
    assert!(entries(&json!({})).is_empty());
}

#[test]
fn scheme_hotkeys_replace_the_shown_key_of_checked_rows_only() {
    // SHOWN-04: with another scheme the popups show what the scheme gives
    let rows = rows(
        &inventory(),
        &[entry("firefox.desktop", Some("a"))],
        &foreign(),
    );
    let labels = [Some("1".to_owned())];
    let rows = with_scheme_hotkeys(rows, &labels);
    assert_eq!(rows[0].shown_hotkey.as_deref(), Some("1"));
    assert_eq!(rows[0].hotkey.as_deref(), Some("a"));
    assert_eq!(rows[1].shown_hotkey, None);
}
