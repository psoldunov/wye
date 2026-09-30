use serde_json::json;
use wye_core::config::PickerKeys;
use wye_core::merge_patch::apply_to_config;
use wye_core::{Modifier, Modifiers};

use super::*;

fn binding(text: &str) -> KeyBinding {
    parse(text).expect("a binding")
}

fn applied(config: &Config, patch: &Value) -> Config {
    apply_to_config(config, patch).expect("a valid configuration")
}

fn set(modifiers: &[Modifier]) -> Modifiers {
    Modifiers::from_slice(modifiers)
}

#[test]
fn a_free_binding_is_free() {
    // KEY-02
    let check =
        check_binding(&Config::default(), &[], "open", &binding("Ctrl+j")).expect("known action");
    assert_eq!(check, Check::Free);
}

#[test]
fn a_binding_the_action_has_is_a_duplicate() {
    let check =
        check_binding(&Config::default(), &[], "open", &binding("Return")).expect("known action");
    assert_eq!(check, Check::Duplicate);
}

#[test]
fn a_binding_another_action_uses_names_that_action() {
    // KEY-21
    let check =
        check_binding(&Config::default(), &[], "cancel", &binding("Ctrl+c")).expect("known action");
    let Check::Clash(clash) = check else {
        panic!("expected a clash: {check:?}");
    };
    assert_eq!(clash.message(), "Already used by “Copy link and close”");
}

#[test]
fn a_plain_key_that_is_a_target_hotkey_clashes_with_the_target() {
    // KEY-12, KEY-21
    let hotkeys = [HotkeyUse {
        key: "f".to_owned(),
        name: "Firefox".to_owned(),
    }];
    let check =
        check_binding(&Config::default(), &hotkeys, "open", &binding("f")).expect("known action");
    assert_eq!(check, Check::Clash(Clash::Target("Firefox".to_owned())));
    // With a modifier it is a different binding.
    let check = check_binding(&Config::default(), &hotkeys, "open", &binding("Ctrl+f"))
        .expect("known action");
    assert_eq!(check, Check::Free);
}

#[test]
fn an_unknown_action_is_refused() {
    assert!(check_binding(&Config::default(), &[], "nope", &binding("a")).is_err());
    assert!(bind_patch(&Config::default(), &[], "nope", &binding("a"), false).is_err());
}

/// The picker keys after `action` takes `text`.
fn keys_after(action: &str, text: &str, replace: bool) -> PickerKeys {
    let config = Config::default();
    let patch = bind_patch(&config, &[], action, &binding(text), replace).expect("patch");
    applied(&config, &patch).picker.keys
}

#[test]
fn adding_a_binding_appends_it_in_stored_form() {
    // KEY-02, KEY-03
    assert_eq!(
        keys_after("cancel", "ctrl+shift+K", false).cancel,
        ["Escape", "Ctrl+Shift+k"]
    );
}

#[test]
fn adding_a_binding_twice_changes_nothing() {
    assert_eq!(keys_after("cancel", "Escape", false).cancel, ["Escape"]);
}

#[test]
fn replacing_takes_the_binding_from_the_other_action() {
    // KEY-21: Replace
    let keys = keys_after("cancel", "Ctrl+c", true);
    assert_eq!(keys.cancel, ["Escape", "Ctrl+c"]);
    assert!(keys.copy_link.is_empty());
}

#[test]
fn replacing_clears_a_target_hotkey() {
    // KEY-12, KEY-21
    let shown = [
        Entry {
            target: json!({"app": "firefox.desktop"}),
            hotkey: Some("f".to_owned()),
        },
        Entry {
            target: json!({"app": "chrome.desktop"}),
            hotkey: Some("c".to_owned()),
        },
    ];
    let patch = bind_patch(&Config::default(), &shown, "open", &binding("f"), true).expect("patch");
    assert_eq!(
        patch["browsers"]["shown"],
        json!([{"target": {"app": "firefox.desktop"}}, {"target": {"app": "chrome.desktop"}, "hotkey": "c"}])
    );
    assert!(
        patch["picker"]["keys"]["open"]
            .as_array()
            .is_some_and(|list| list.contains(&json!("f")))
    );
}

/// What stops the default configuration's `which` from taking `pressed`.
fn stopped(which: &str, pressed: &[Modifier]) -> Option<Clash> {
    check_modifiers(&Config::default(), which, set(pressed)).expect("known")
}

/// The picker keys after `which` takes `pressed`.
fn modifiers_after(which: &str, pressed: &[Modifier], replace: bool) -> (Value, PickerKeys) {
    let config = Config::default();
    let patch = modifiers_patch(&config, which, set(pressed), replace).expect("patch");
    let keys = applied(&config, &patch).picker.keys;
    (patch, keys)
}

#[test]
fn modifier_actions_need_different_sets() {
    // KEY-21
    assert_eq!(
        stopped("background-modifier", &[Modifier::Shift]),
        Some(Clash::Modifier("Open in private window".to_owned()))
    );
    assert_eq!(stopped("background-modifier", &[Modifier::Super]), None);
}

#[test]
fn no_modifier_pressed_is_always_free() {
    // KEY-01
    assert_eq!(stopped("private-modifier", &[]), None);
}

#[test]
fn a_set_the_action_has_already_is_free() {
    assert_eq!(stopped("private-modifier", &[Modifier::Shift]), None);
}

#[test]
fn replacing_a_modifier_set_clears_the_other_action() {
    // KEY-21
    let (_, keys) = modifiers_after("background-modifier", &[Modifier::Shift], true);
    assert_eq!(keys.background_modifier, set(&[Modifier::Shift]));
    assert!(keys.private_modifier.is_empty());
}

#[test]
fn a_modifier_set_without_replacing_touches_only_its_own_key() {
    let (patch, _) = modifiers_after("new-window-modifier", &[Modifier::Super], false);
    assert_eq!(
        patch,
        json!({"picker": {"keys": {"new-window-modifier": ["Super"]}}})
    );
}

#[test]
fn an_unknown_modifier_action_is_refused() {
    assert!(check_modifiers(&Config::default(), "open", Modifiers::NONE).is_err());
    assert!(modifiers_patch(&Config::default(), "open", Modifiers::NONE, false).is_err());
}
