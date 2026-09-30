//! The fallback for non-Latin layouts (KEY-11): a hotkey matches the
//! character the active layout produces, or the Latin character on the same
//! physical key. The second half needs the key's position, which is an evdev
//! scancode; this is the US-QWERTY table for it.

/// Qt's `nativeScanCode` and X11 keycodes are the evdev code plus this.
pub const EVDEV_OFFSET: u32 = 8;

/// `(evdev code, unshifted US-QWERTY character)`.
const US_QWERTY: &[(u32, char)] = &[
    (2, '1'),
    (3, '2'),
    (4, '3'),
    (5, '4'),
    (6, '5'),
    (7, '6'),
    (8, '7'),
    (9, '8'),
    (10, '9'),
    (11, '0'),
    (12, '-'),
    (13, '='),
    (16, 'q'),
    (17, 'w'),
    (18, 'e'),
    (19, 'r'),
    (20, 't'),
    (21, 'y'),
    (22, 'u'),
    (23, 'i'),
    (24, 'o'),
    (25, 'p'),
    (26, '['),
    (27, ']'),
    (30, 'a'),
    (31, 's'),
    (32, 'd'),
    (33, 'f'),
    (34, 'g'),
    (35, 'h'),
    (36, 'j'),
    (37, 'k'),
    (38, 'l'),
    (39, ';'),
    (40, '\''),
    (41, '`'),
    (43, '\\'),
    (44, 'z'),
    (45, 'x'),
    (46, 'c'),
    (47, 'v'),
    (48, 'b'),
    (49, 'n'),
    (50, 'm'),
    (51, ','),
    (52, '.'),
    (53, '/'),
    // Keypad digits, so a hotkey of "1" also answers the keypad's 1.
    (71, '7'),
    (72, '8'),
    (73, '9'),
    (75, '4'),
    (76, '5'),
    (77, '6'),
    (79, '1'),
    (80, '2'),
    (81, '3'),
    (82, '0'),
];

/// The US-QWERTY character on the key with this evdev code.
#[must_use]
pub fn latin_from_evdev(code: u32) -> Option<char> {
    US_QWERTY
        .iter()
        .find(|(evdev, _)| *evdev == code)
        .map(|&(_, c)| c)
}

/// The same from Qt's `nativeScanCode` (evdev code + 8).
#[must_use]
pub fn latin_from_native_scancode(code: u32) -> Option<char> {
    code.checked_sub(EVDEV_OFFSET).and_then(latin_from_evdev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_letter_digit_and_punctuation_rows() {
        assert_eq!(latin_from_evdev(16), Some('q'));
        assert_eq!(latin_from_evdev(30), Some('a'));
        assert_eq!(latin_from_evdev(44), Some('z'));
        assert_eq!(latin_from_evdev(2), Some('1'));
        assert_eq!(latin_from_evdev(11), Some('0'));
        assert_eq!(latin_from_evdev(51), Some(','));
        assert_eq!(latin_from_evdev(57), None, "space is not in the table");
        assert_eq!(latin_from_evdev(0), None);
    }

    #[test]
    fn native_scancodes_are_shifted_by_eight() {
        // KEY_A is evdev 30, so Qt reports 38 for it.
        assert_eq!(latin_from_native_scancode(38), Some('a'));
        assert_eq!(latin_from_native_scancode(24), Some('q'));
        assert_eq!(latin_from_native_scancode(7), None);
        assert_eq!(latin_from_native_scancode(0), None);
    }

    #[test]
    fn every_letter_key_is_present_once() {
        let letters: Vec<char> = US_QWERTY
            .iter()
            .map(|&(_, c)| c)
            .filter(char::is_ascii_lowercase)
            .collect();
        assert_eq!(letters.len(), 26);
        for c in 'a'..='z' {
            assert!(letters.contains(&c), "{c}");
        }
    }

    #[test]
    fn codes_are_unique() {
        let mut codes: Vec<u32> = US_QWERTY.iter().map(|&(code, _)| code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), US_QWERTY.len());
    }
}
