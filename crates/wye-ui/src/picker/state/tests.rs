use super::super::fixture;
use super::super::keys::QtKey;
use super::*;

const SHIFT: u32 = 0x0200_0000;
const CONTROL: u32 = 0x0400_0000;
const KEY_SHIFT: u32 = 0x0100_0020;
const KEY_RIGHT: u32 = 0x0100_0014;
const KEY_RETURN: u32 = 0x0100_0004;
const KEY_ESCAPE: u32 = 0x0100_0000;

fn key(code: u32, text: &str, modifiers: u32) -> QtKey {
    QtKey {
        key: code,
        text: text.to_owned(),
        native_scancode: 0,
        modifiers,
    }
}

fn state() -> PickerState {
    PickerState::new(fixture::view())
}

fn chosen(effect: &Effect) -> &ChoiceOut {
    match effect {
        Effect::Choose(choice) => choice,
        other => panic!("expected a choice, got {other:?}"),
    }
}

#[test]
fn the_first_tile_is_selected_and_arrows_move() {
    // PICK-24, PICK-22.
    let state = state();
    assert_eq!(state.selected, 0);
    let (state, effect) = state.key(&key(KEY_RIGHT, "", 0));
    assert_eq!((state.selected, effect), (1, Effect::None));
    let (state, _) = state.key(&key(KEY_RIGHT, "", 0));
    assert_eq!(state.selected, 0, "wraps around");
}

#[test]
fn enter_opens_the_selected_tile() {
    // PICK-22.
    let (state, _) = state().key(&key(KEY_RIGHT, "", 0));
    let (_, effect) = state.key(&key(KEY_RETURN, "\r", 0));
    let choice = chosen(&effect);
    assert!(choice.target.contains("Profile 1"), "{choice:?}");
    assert_eq!(choice.app_id, "google-chrome");
}

#[test]
fn a_hotkey_opens_its_tile_case_insensitively() {
    // PICK-21, KEY-11.
    let (_, effect) = state().key(&key(u32::from('F'), "F", SHIFT));
    // Shift is the private-window modifier: Firefox opens privately.
    let choice = chosen(&effect);
    assert!(choice.target.contains("private"), "{choice:?}");
}

#[test]
fn held_shift_shows_the_hint_and_dims_what_cannot_open_privately() {
    // KEY-13, PICK-14.
    let (state, effect) = state().key(&key(KEY_SHIFT, "", SHIFT));
    assert_eq!(effect, Effect::None);
    assert_eq!(state.hint(), "Open in a private window");
    assert_eq!(state.dimmed(), vec![false, true]);
    let released = state.with_held(wye_core::Modifiers::NONE);
    assert_eq!(released.hint(), "");
}

#[test]
fn clicks_follow_held_modifiers_and_middle_opens_in_the_background() {
    // PICK-20, PICK-32, PICK-33.
    let state = state();
    let plain = state.activate(0, false);
    assert_eq!(chosen(&plain).target, r#"{"app":"firefox.desktop"}"#);
    assert!(!chosen(&plain).background);
    assert!(chosen(&state.activate(0, true)).background);
    let held = state.with_held(wye_core::Modifiers::from_slice(&[wye_core::Modifier::Ctrl]));
    assert!(chosen(&held.activate(1, false)).background);
}

#[test]
fn escape_copy_and_create_rule_are_actions() {
    // PICK-23, KEY-22, PICK-31.
    assert_eq!(state().key(&key(KEY_ESCAPE, "", 0)).1, Effect::Cancel);
    assert_eq!(
        state().key(&key(u32::from('C'), "c", CONTROL)).1,
        Effect::CopyLink
    );
    assert_eq!(
        state().key(&key(u32::from('R'), "r", CONTROL)).1,
        Effect::CreateRule
    );
    assert_eq!(state().key(&key(u32::from('Q'), "q", 0)).1, Effect::Ignored);
}

#[test]
fn open_in_chooses_a_target_that_is_not_a_tile() {
    // PICK-28.
    let effect = state().open_in(0, 0);
    assert_eq!(chosen(&effect).target, r#"{"app":"brave.desktop"}"#);
    assert_eq!(state().open_in(3, 0), Effect::Ignored);
}

#[test]
fn the_tile_menu_offers_only_what_the_target_supports() {
    // PICK-30.
    let firefox: Vec<_> = state()
        .tile_menu(0)
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    assert!(firefox.contains(&"open-private".to_owned()));
    let work: Vec<_> = state()
        .tile_menu(1)
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    assert!(!work.contains(&"open-private".to_owned()));
    assert!(work.contains(&"make-primary".to_owned()));
    assert!(matches!(
        state().tile_action(1, "make-primary"),
        Effect::MakePrimary(_)
    ));
    assert!(chosen(&state().tile_action(0, "open-new-window")).new_window);
}

#[test]
fn hovering_selects_but_only_real_tiles() {
    assert_eq!(state().hover(1).selected, 1);
    assert_eq!(state().hover(7).selected, 0);
}
