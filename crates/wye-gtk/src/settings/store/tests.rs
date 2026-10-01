use serde_json::json;

use super::*;
use crate::settings::menu::Surface;

/// A writable store on `config`, as a fixture loads it.
pub(crate) fn store_with(config: &Value) -> SettingsStore {
    let store = SettingsStore::new();
    store
        .load_fixture(&json!({"config": config, "status": {"config": {"writable": true}}}))
        .expect("a fixture");
    store
}

#[test]
fn values_are_read_by_dotted_path() {
    let store = store_with(&json!({"general": {"tray-icon": "wye", "show-tray-icon": false}}));
    assert_eq!(
        store.string_value("general.tray-icon", "primary-browser"),
        "wye"
    );
    assert!(!store.bool_value("general.show-tray-icon", true));
    assert!(store.bool_value("general.launch-at-login", true), "default");
    assert!(store.writable());
    assert!(store.loaded());
    assert!(store.offline());
}

#[test]
fn set_06_a_change_shows_at_once_and_stays_local_offline() {
    let store = store_with(&json!({"general": {"show-tray-icon": true}}));
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    store.connect_changed(glib::clone!(
        #[strong]
        seen,
        move |_| seen.set(seen.get() + 1)
    ));
    store.set_value("general.show-tray-icon", json!(false));
    assert!(!store.bool_value("general.show-tray-icon", true));
    assert_eq!(seen.get(), 1);
    assert!(store.error().is_empty());
}

#[test]
fn loading_shows_loaded_already() {
    // A widget that reads `loaded` in its `changed` callback (the
    // callout, the default-browser button) sees it true.
    let store = SettingsStore::new();
    let seen = std::rc::Rc::new(std::cell::Cell::new(false));
    store.connect_changed(glib::clone!(
        #[strong]
        seen,
        move |store| seen.set(store.loaded())
    ));
    store.load_fixture(&json!({})).expect("a fixture");
    assert!(seen.get());
}

#[test]
fn a_bad_path_shows_an_error_and_changes_nothing() {
    let store = store_with(&json!({}));
    store.set_value("General.X", json!(1));
    assert!(!store.error().is_empty());
    store.clear_error();
    assert!(store.error().is_empty());
}

#[test]
fn blk_09_a_dismissed_callout_stays_dismissed() {
    let store = store_with(&json!({}));
    assert!(!store.callout_dismissed("general-links"));
    store.dismiss_callout("general-links");
    assert!(store.callout_dismissed("general-links"));
}

#[test]
fn set_08_the_last_page_is_remembered() {
    let store = store_with(&json!({}));
    store.remember_page("rules");
    assert_eq!(
        store.with_snapshot(|s| s.status.ui_state.last_page.clone()),
        Some("rules".to_owned())
    );
}

#[test]
fn tgt_01_the_closed_row_names_the_current_target() {
    let store = SettingsStore::new();
    store
        .load_fixture(&json!({
            "config": {"browsers": {"primary": {"app": "firefox.desktop"}}},
            "targets": {"targets": [
                {"target": {"picker": true}, "kind": "picker", "name": "Picker"},
                {"target": {"app": "firefox.desktop"}, "kind": "app", "name": "Firefox", "icon": "firefox"}
            ]}
        }))
        .expect("fixture");
    let current = store.value("browsers.primary").expect("set");
    let row = store.target_label(Surface::Browsers, &current, "");
    assert_eq!(
        (row.label.as_str(), row.icon.as_str()),
        ("Firefox", "firefox")
    );
    let rows = store.target_menu(Surface::Browsers, &current, "");
    assert!(rows.iter().any(|row| row.checked && row.label == "Firefox"));
}

#[test]
fn blk_08_help_texts_name_the_alternative_key() {
    let store = store_with(&json!({"browsers": {"alternative-key": ["Shift"]}}));
    assert!(
        store
            .help_text("alternative-browser")
            .starts_with("Hold Shift")
    );
    assert_eq!(store.help_text("nope"), "");
}

#[test]
fn a_handler_goes_with_its_owner() {
    // A row built again must not stay connected, with what it captured.
    let store = store_with(&json!({}));
    let captured = std::rc::Rc::new(std::cell::Cell::new(0));
    let owner = glib::Object::new::<glib::Object>();
    store.connect_changed_while(
        &owner,
        glib::clone!(
            #[strong]
            captured,
            move |_| captured.set(captured.get() + 1)
        ),
    );
    store.set_value("general.show-tray-icon", json!(false));
    assert_eq!(captured.get(), 1);
    assert_eq!(std::rc::Rc::strong_count(&captured), 2);
    drop(owner);
    store.set_value("general.show-tray-icon", json!(true));
    assert_eq!(captured.get(), 1, "not run once the owner is gone");
    assert_eq!(
        std::rc::Rc::strong_count(&captured),
        1,
        "what the handler captured is released"
    );
}
