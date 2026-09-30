//! Qt key events as the core keymap reads them (KEY-03, KEY-05, KEY-11).
//!
//! QML hands over `event.key`, `event.text`, `event.nativeScanCode` and
//! `event.modifiers`; the key becomes an XKB key name where Qt's code has
//! one, the text and scan code travel along for layout-independent hotkeys.

use wye_core::keybinding::KeyEvent;
use wye_core::{Modifier, Modifiers};

/// `Qt::KeyboardModifier` bits.
const SHIFT: u32 = 0x0200_0000;
const CONTROL: u32 = 0x0400_0000;
const ALT: u32 = 0x0800_0000;
/// The Super (Windows) key on Linux.
const META: u32 = 0x1000_0000;
const KEYPAD: u32 = 0x2000_0000;

/// `Qt::Key_Enter`, the keypad's Enter.
const KEY_ENTER: u32 = 0x0100_0005;

/// `Qt::Key` codes with an XKB name (KEY-03).
const NAMED: &[(u32, &str)] = &[
    (0x0100_0000, "Escape"),
    (0x0100_0001, "Tab"),
    // Shift+Tab arrives as Backtab.
    (0x0100_0002, "ISO_Left_Tab"),
    (0x0100_0003, "BackSpace"),
    (0x0100_0004, "Return"),
    (KEY_ENTER, "Return"),
    (0x0100_0006, "Insert"),
    (0x0100_0007, "Delete"),
    (0x0100_0008, "Pause"),
    (0x0100_0009, "Print"),
    (0x0100_0010, "Home"),
    (0x0100_0011, "End"),
    (0x0100_0012, "Left"),
    (0x0100_0013, "Up"),
    (0x0100_0014, "Right"),
    (0x0100_0015, "Down"),
    (0x0100_0016, "Page_Up"),
    (0x0100_0017, "Page_Down"),
    (0x0100_0055, "Menu"),
    (0x20, "space"),
];

/// `Qt::Key` codes of the modifier keys themselves: pressing or releasing
/// one only changes what is held (KEY-13).
const MODIFIER_KEYS: &[u32] = &[
    0x0100_0020, // Shift
    0x0100_0021, // Control
    0x0100_0022, // Meta
    0x0100_0023, // Alt
    0x0100_0024, // CapsLock
    0x0100_0025, // NumLock
    0x0100_0053, // Super_L
    0x0100_0054, // Super_R
    0x0100_1103, // AltGr
];

/// What QML reports for one key press.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QtKey {
    pub key: u32,
    pub text: String,
    pub native_scancode: u32,
    pub modifiers: u32,
}

impl QtKey {
    /// Whether this is a modifier key on its own.
    pub fn is_modifier(&self) -> bool {
        MODIFIER_KEYS.contains(&self.key)
    }

    /// The core's view of the press.
    pub fn event(&self) -> KeyEvent {
        KeyEvent {
            key: name(self.key, self.modifiers),
            text: printable(&self.text),
            native_scancode: (self.native_scancode != 0).then_some(self.native_scancode),
            modifiers: modifiers(self.modifiers),
        }
    }
}

/// The modifiers in `Qt::KeyboardModifiers` bits; left and right alike
/// (KEY-01), the keypad flag ignored.
pub fn modifiers(bits: u32) -> Modifiers {
    let held: Vec<Modifier> = [
        (SHIFT, Modifier::Shift),
        (CONTROL, Modifier::Ctrl),
        (ALT, Modifier::Alt),
        (META, Modifier::Super),
    ]
    .into_iter()
    .filter(|(bit, _)| bits & bit != 0)
    .map(|(_, modifier)| modifier)
    .collect();
    Modifiers::from_slice(&held)
}

/// The XKB name of Qt key `code`, when it has one: the named keys, the
/// keypad's Enter, and Latin letters and digits (Qt reports letters in upper
/// case).
fn name(code: u32, bits: u32) -> Option<String> {
    if code == KEY_ENTER && bits & KEYPAD != 0 {
        return Some("KP_Enter".to_owned());
    }
    if let Some((_, name)) = NAMED.iter().find(|(named, _)| *named == code) {
        return Some((*name).to_owned());
    }
    char::from_u32(code)
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase().to_string())
}

/// Text a key produced, without control characters (Return gives `\r`).
fn printable(text: &str) -> Option<String> {
    Some(text.to_owned()).filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: u32, text: &str, modifiers: u32) -> KeyEvent {
        QtKey {
            key,
            text: text.to_owned(),
            native_scancode: 0,
            modifiers,
        }
        .event()
    }

    #[test]
    fn named_keys_get_their_xkb_names() {
        assert_eq!(press(0x0100_0004, "\r", 0).key.as_deref(), Some("Return"));
        assert_eq!(press(0x0100_0004, "\r", 0).text, None);
        assert_eq!(press(0x0100_0000, "", 0).key.as_deref(), Some("Escape"));
        assert_eq!(press(0x20, " ", 0).key.as_deref(), Some("space"));
    }

    #[test]
    fn the_keypad_enter_is_its_own_key() {
        assert_eq!(
            press(KEY_ENTER, "\r", KEYPAD).key.as_deref(),
            Some("KP_Enter")
        );
    }

    #[test]
    fn letters_are_lower_case_and_keep_their_text() {
        let event = press(u32::from('F'), "f", 0);
        assert_eq!(event.key.as_deref(), Some("f"));
        assert_eq!(event.text.as_deref(), Some("f"));
    }

    #[test]
    fn non_latin_keys_travel_as_text() {
        // KEY-11: the core also tries the Latin key at the same position.
        let event = QtKey {
            key: u32::from('Ф'),
            text: "ф".into(),
            native_scancode: 41,
            modifiers: 0,
        }
        .event();
        assert_eq!(event.key, None);
        assert_eq!(event.text.as_deref(), Some("ф"));
        assert_eq!(event.native_scancode, Some(41));
    }

    #[test]
    fn modifier_bits_map_left_and_right_alike() {
        let held = modifiers(SHIFT | META | KEYPAD);
        assert!(held.contains(Modifier::Shift));
        assert!(held.contains(Modifier::Super));
        assert!(!held.contains(Modifier::Ctrl));
        assert!(
            QtKey {
                key: 0x0100_0020,
                ..QtKey::default()
            }
            .is_modifier()
        );
    }

    #[test]
    fn shift_tab_reads_as_tab_with_shift() {
        let event = press(0x0100_0002, "", SHIFT);
        let binding: wye_core::keybinding::KeyBinding = "Shift+Tab".parse().expect("binding");
        assert!(binding.matches(&event));
    }
}
