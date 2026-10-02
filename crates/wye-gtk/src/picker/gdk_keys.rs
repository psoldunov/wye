//! GDK key events in the shape the shared picker logic reads (KEY-03,
//! KEY-05, KEY-11, KEY-13).
//!
//! `keys.rs` and `state.rs` are crates/wye-ui's, compiled here from the same
//! files with their tests, and they read a press as Qt reports it: a
//! `Qt::Key` code, the text, the native scan code and `Qt::KeyboardModifiers`
//! bits. This module turns a GDK press into exactly that, so both pickers
//! answer every key through one path: XKB key names to Qt's codes for the
//! keys the bindings name, letters and digits as the upper-case character
//! (Qt's convention), the keypad flag on `KP_` keys, and GDK's hardware key
//! code, which is XKB's (evdev plus 8) like Qt's native scan code.

use gtk::gdk;

use super::keys::QtKey;

/// `Qt::KeyboardModifier` bits.
const SHIFT: u32 = 0x0200_0000;
const CONTROL: u32 = 0x0400_0000;
const ALT: u32 = 0x0800_0000;
const META: u32 = 0x1000_0000;
const KEYPAD: u32 = 0x2000_0000;

/// XKB key names with the `Qt::Key` code Qt gives the key: the keys the
/// picker's bindings name, and the modifier keys (KEY-13).
const QT_CODES: &[(&str, u32)] = &[
    ("Escape", 0x0100_0000),
    ("Tab", 0x0100_0001),
    ("ISO_Left_Tab", 0x0100_0002),
    ("BackSpace", 0x0100_0003),
    ("Return", 0x0100_0004),
    ("KP_Enter", 0x0100_0005),
    ("Insert", 0x0100_0006),
    ("Delete", 0x0100_0007),
    ("Pause", 0x0100_0008),
    ("Print", 0x0100_0009),
    ("Home", 0x0100_0010),
    ("End", 0x0100_0011),
    ("Left", 0x0100_0012),
    ("Up", 0x0100_0013),
    ("Right", 0x0100_0014),
    ("Down", 0x0100_0015),
    ("Page_Up", 0x0100_0016),
    ("Page_Down", 0x0100_0017),
    ("Menu", 0x0100_0055),
    ("space", 0x20),
    ("Shift_L", 0x0100_0020),
    ("Shift_R", 0x0100_0020),
    ("Control_L", 0x0100_0021),
    ("Control_R", 0x0100_0021),
    ("Meta_L", 0x0100_0022),
    ("Meta_R", 0x0100_0022),
    ("Alt_L", 0x0100_0023),
    ("Alt_R", 0x0100_0023),
    ("Caps_Lock", 0x0100_0024),
    ("Num_Lock", 0x0100_0025),
    ("Super_L", 0x0100_0053),
    ("Super_R", 0x0100_0054),
    ("ISO_Level3_Shift", 0x0100_1103),
];

/// The press GTK reported to an `EventControllerKey`.
pub fn press(keyval: gdk::Key, keycode: u32, state: gdk::ModifierType) -> QtKey {
    let name = keyval
        .name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let keypad = if name.starts_with("KP_") { KEYPAD } else { 0 };
    let character = keyval.to_unicode();
    let key = QT_CODES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, code)| *code)
        .or_else(|| {
            character
                .and_then(|c| c.to_uppercase().next())
                .map(u32::from)
        })
        .unwrap_or_default();
    QtKey {
        key,
        text: character.map(String::from).unwrap_or_default(),
        native_scancode: keycode,
        modifiers: modifier_bits(state) | keypad,
    }
}

/// What is held once a modifier key's press (or release) has taken effect
/// (KEY-13). GDK reports the state from before the event, and so does the
/// key controller's `modifiers` signal: on Shift's press, Shift is not yet
/// in it, on its release it still is.
pub fn after(keyval: gdk::Key, state: gdk::ModifierType, pressed: bool) -> gdk::ModifierType {
    let name = keyval
        .name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let mask = match name.as_str() {
        "Shift_L" | "Shift_R" => gdk::ModifierType::SHIFT_MASK,
        "Control_L" | "Control_R" => gdk::ModifierType::CONTROL_MASK,
        "Alt_L" | "Alt_R" | "Meta_L" | "Meta_R" => gdk::ModifierType::ALT_MASK,
        "Super_L" | "Super_R" => gdk::ModifierType::SUPER_MASK,
        _ => return state,
    };
    if pressed { state | mask } else { state - mask }
}

/// A GDK modifier state as `Qt::KeyboardModifiers` bits; left and right
/// alike (KEY-01). Super arrives as `SUPER` on Wayland and may be `META` on
/// X11.
pub fn modifier_bits(state: gdk::ModifierType) -> u32 {
    [
        (gdk::ModifierType::SHIFT_MASK, SHIFT),
        (gdk::ModifierType::CONTROL_MASK, CONTROL),
        (gdk::ModifierType::ALT_MASK, ALT),
        (
            gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK,
            META,
        ),
    ]
    .into_iter()
    .filter(|(mask, _)| state.intersects(*mask))
    .fold(0, |bits, (_, bit)| bits | bit)
}

#[cfg(test)]
mod tests {
    use gtk::gdk::{Key, ModifierType};
    use wye_core::Modifier;

    use super::super::fixture;
    use super::super::keys;
    use super::super::state::{Effect, PickerState};
    use super::*;

    fn event(keyval: Key, modifiers: ModifierType) -> wye_core::keybinding::KeyEvent {
        press(keyval, 0, modifiers).event()
    }

    #[test]
    fn named_keys_read_as_their_xkb_names() {
        let none = ModifierType::empty();
        assert_eq!(event(Key::Return, none).key.as_deref(), Some("Return"));
        assert_eq!(event(Key::KP_Enter, none).key.as_deref(), Some("KP_Enter"));
        assert_eq!(event(Key::Escape, none).key.as_deref(), Some("Escape"));
        assert_eq!(event(Key::space, none).key.as_deref(), Some("space"));
        assert_eq!(event(Key::Menu, none).key.as_deref(), Some("Menu"));
    }

    #[test]
    fn letters_digits_and_keypad_digits_read_in_lower_case() {
        // KEY-11: GDK reports Shift+f as `F`.
        let shifted = event(Key::F, ModifierType::SHIFT_MASK);
        assert_eq!(shifted.key.as_deref(), Some("f"));
        assert!(shifted.modifiers.contains(Modifier::Shift));
        assert_eq!(
            event(Key::KP_1, ModifierType::empty()).key.as_deref(),
            Some("1")
        );
    }

    #[test]
    fn shift_tab_matches_the_shift_tab_binding() {
        let binding: wye_core::keybinding::KeyBinding = "Shift+Tab".parse().expect("binding");
        assert!(binding.matches(&event(Key::ISO_Left_Tab, ModifierType::SHIFT_MASK)));
    }

    #[test]
    fn modifier_keys_are_modifiers_and_super_is_super() {
        assert!(press(Key::Shift_R, 0, ModifierType::empty()).is_modifier());
        assert!(press(Key::Super_L, 0, ModifierType::empty()).is_modifier());
        assert!(!press(Key::a, 0, ModifierType::empty()).is_modifier());
        for super_mask in [ModifierType::SUPER_MASK, ModifierType::META_MASK] {
            assert!(keys::modifiers(modifier_bits(super_mask)).contains(Modifier::Super));
        }
        assert_eq!(
            keys::modifiers(modifier_bits(ModifierType::LOCK_MASK)),
            wye_core::Modifiers::NONE
        );
    }

    #[test]
    fn a_modifier_counts_from_its_press_until_its_release() {
        // KEY-13, PICK-14: the hint shows while Shift is down.
        let none = ModifierType::empty();
        let shift = ModifierType::SHIFT_MASK;
        assert_eq!(after(Key::Shift_L, none, true), shift);
        assert_eq!(after(Key::Shift_R, shift, false), none);
        assert_eq!(after(Key::Super_L, none, true), ModifierType::SUPER_MASK);
        assert_eq!(after(Key::a, shift, true), shift, "not a modifier key");
        let state = PickerState::new(fixture::view());
        let held = keys::modifiers(modifier_bits(after(Key::Shift_L, none, true)));
        assert_eq!(state.with_held(held).hint(), "Open in a private window");
    }

    #[test]
    fn a_hotkey_on_another_layout_matches_by_key_code() {
        // KEY-11: `ф` sits where `f` does (XKB key code 41); Firefox opens.
        let state = PickerState::new(fixture::view());
        let (_, effect) = state.key(&press(Key::Cyrillic_ef, 41, ModifierType::empty()));
        let Effect::Choose(choice) = effect else {
            panic!("expected a choice, got {effect:?}");
        };
        assert_eq!(choice.target, r#"{"app":"firefox.desktop"}"#);
    }

    #[test]
    fn ctrl_r_creates_a_rule() {
        // PICK-31.
        let state = PickerState::new(fixture::view());
        let (_, effect) = state.key(&press(Key::r, 0, ModifierType::CONTROL_MASK));
        assert_eq!(effect, Effect::CreateRule);
    }
}
