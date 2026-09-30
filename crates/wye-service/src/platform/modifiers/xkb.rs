//! A `wl_keyboard.modifiers` mask as Wye's modifiers, read with the
//! compositor's own keymap through xkbcommon (KEY-01: left and right count
//! as the same key, which the mask already folds together).

use wye_api::context::Modifier;
use xkbcommon::xkb;

use crate::platform::PlatformError;

/// The masks of one `wl_keyboard.modifiers` event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mask {
    /// Modifiers whose keys are down.
    pub depressed: u32,
    /// Modifiers latched by sticky keys: they apply to the next key.
    pub latched: u32,
    /// Locked modifiers (Caps Lock, Num Lock, a locked Shift).
    pub locked: u32,
    /// Keyboard layout group.
    pub group: u32,
}

/// Each modifier's names in a keymap: the real modifier, then the virtual
/// one that newer keymaps also report.
const NAMES: [(Modifier, &[&str]); 4] = [
    (Modifier::Shift, &[xkb::MOD_NAME_SHIFT]),
    (Modifier::Ctrl, &[xkb::MOD_NAME_CTRL]),
    (Modifier::Alt, &[xkb::MOD_NAME_ALT, "Alt"]),
    (Modifier::Super, &[xkb::MOD_NAME_LOGO, "Super"]),
];

/// The modifiers held in `mask`, reading bits with `keymap` (the text of a
/// `wl_keyboard.keymap`). Held means pressed or latched; a lock (Shift
/// Lock, Caps Lock) is not a held key.
///
/// # Errors
///
/// [`PlatformError::Failed`] when the keymap does not compile.
pub fn held(keymap: &str, mask: Mask) -> Result<Vec<Modifier>, PlatformError> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_DEFAULT_INCLUDES);
    let keymap = xkb::Keymap::new_from_string(
        &context,
        keymap.to_owned(),
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or_else(|| PlatformError::Failed("the compositor's keymap does not compile".to_owned()))?;
    let mut state = xkb::State::new(&keymap);
    state.update_mask(mask.depressed, mask.latched, mask.locked, 0, 0, mask.group);
    let held_now = xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED;
    Ok(NAMES
        .iter()
        .filter(|(_, names)| {
            names
                .iter()
                .any(|name| state.mod_name_is_active(name, held_now))
        })
        .map(|(modifier, _)| *modifier)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete, self-contained keymap: one key per modifier, mapped to
    /// the real modifiers the way `evdev` keymaps do.
    const KEYMAP: &str = r#"
xkb_keymap {
    xkb_keycodes "test" {
        minimum = 8;
        maximum = 255;
        <LFSH> = 50;
        <RTSH> = 62;
        <LCTL> = 37;
        <LALT> = 64;
        <LWIN> = 133;
        <CAPS> = 66;
    };
    xkb_types "test" {
        virtual_modifiers Alt,Super;
        type "ONE_LEVEL" {
            modifiers = none;
            map[none] = Level1;
            level_name[Level1] = "Any";
        };
    };
    xkb_compat "test" {
        virtual_modifiers Alt,Super;
        interpret Shift_L { action = SetMods(modifiers = Shift); };
        interpret Shift_R { action = SetMods(modifiers = Shift); };
        interpret Control_L { action = SetMods(modifiers = Control); };
        interpret Alt_L { virtualModifier = Alt; action = SetMods(modifiers = modMapMods); };
        interpret Super_L { virtualModifier = Super; action = SetMods(modifiers = modMapMods); };
        interpret Caps_Lock { action = LockMods(modifiers = Lock); };
    };
    xkb_symbols "test" {
        key <LFSH> { [ Shift_L ] };
        key <RTSH> { [ Shift_R ] };
        key <LCTL> { [ Control_L ] };
        key <LALT> { [ Alt_L ] };
        key <LWIN> { [ Super_L ] };
        key <CAPS> { [ Caps_Lock ] };
        modifier_map Shift { <LFSH>, <RTSH> };
        modifier_map Control { <LCTL> };
        modifier_map Mod1 { <LALT> };
        modifier_map Mod4 { <LWIN> };
        modifier_map Lock { <CAPS> };
    };
};
"#;

    /// The mask bit of the modifier named `name` in [`KEYMAP`].
    fn bit(name: &str) -> u32 {
        let context = xkb::Context::new(xkb::CONTEXT_NO_DEFAULT_INCLUDES);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            KEYMAP.to_owned(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("the test keymap compiles");
        1 << keymap.mod_get_index(name)
    }

    fn pressed(names: &[&str]) -> Mask {
        Mask {
            depressed: names
                .iter()
                .map(|name| bit(name))
                .fold(0, |mask, bit| mask | bit),
            ..Mask::default()
        }
    }

    #[test]
    fn each_real_modifier_maps_to_its_key_key_06() {
        let cases = [
            ("Shift", Modifier::Shift),
            ("Control", Modifier::Ctrl),
            ("Mod1", Modifier::Alt),
            ("Mod4", Modifier::Super),
        ];
        for (name, modifier) in cases {
            assert_eq!(held(KEYMAP, pressed(&[name])), Ok(vec![modifier]), "{name}");
        }
    }

    #[test]
    fn several_held_keys_come_in_a_fixed_order() {
        let mask = pressed(&["Mod4", "Shift", "Control"]);
        assert_eq!(
            held(KEYMAP, mask),
            Ok(vec![Modifier::Shift, Modifier::Ctrl, Modifier::Super])
        );
    }

    #[test]
    fn nothing_held_is_an_empty_set_not_unknown() {
        assert_eq!(held(KEYMAP, Mask::default()), Ok(Vec::new()));
    }

    #[test]
    fn locks_do_not_count_and_latches_do() {
        let locked = Mask {
            locked: bit("Lock") | bit("Shift"),
            ..Mask::default()
        };
        assert_eq!(held(KEYMAP, locked), Ok(Vec::new()));
        let latched = Mask {
            latched: bit("Control"),
            ..Mask::default()
        };
        assert_eq!(held(KEYMAP, latched), Ok(vec![Modifier::Ctrl]));
    }

    #[test]
    fn a_broken_keymap_is_an_error() {
        assert!(held("xkb_keymap {", Mask::default()).is_err());
    }
}
