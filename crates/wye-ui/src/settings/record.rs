//! Recording a key (KEY-02, BLK-16): a Qt key press as a stored binding.
//!
//! While a recorder listens, every key press goes here. Escape cancels,
//! Backspace clears, a modifier on its own waits for the key it modifies,
//! and anything else is the binding, stored with XKB names (KEY-03).

use serde::Serialize;
use wye_core::Modifiers;
use wye_core::keybinding::{KeyBinding, canonical_key, parse_bindings};

use crate::picker::keys::{QtKey, modifiers};

/// `Qt::Key_F1`; the function keys are consecutive up to F35.
const KEY_F1: u32 = 0x0100_0030;
const KEY_F35: u32 = 0x0100_0052;

/// What a key press did to a recorder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Recorded {
    /// Keep listening: only a modifier is down.
    Pending,
    /// Escape: stop recording, change nothing.
    Cancel,
    /// Backspace: clear the binding.
    Clear,
    /// The binding, as stored and as shown.
    Binding { stored: String, label: String },
}

fn key_name(press: &QtKey) -> Option<String> {
    let event = press.event();
    if let Some(name) = event.key {
        return Some(name);
    }
    if (KEY_F1..=KEY_F35).contains(&press.key) {
        return Some(format!("F{}", press.key - KEY_F1 + 1));
    }
    event.text.and_then(|text| canonical_key(&text).ok())
}

/// The outcome of one key press. With `single` set (a hotkey, KEY-12, or
/// a key with no modifiers) the modifiers are ignored: the key alone is the
/// result.
#[must_use]
pub fn record(press: &QtKey, single: bool) -> Recorded {
    if press.is_modifier() {
        return Recorded::Pending;
    }
    let held = modifiers(press.modifiers);
    match (press.event().key.as_deref(), held.is_empty()) {
        (Some("Escape"), true) => return Recorded::Cancel,
        (Some("BackSpace"), true) => return Recorded::Clear,
        _ => {}
    }
    let Some(name) = key_name(press) else {
        return Recorded::Pending;
    };
    let held = if single { Modifiers::NONE } else { held };
    match KeyBinding::new(held, &name) {
        Ok(binding) => Recorded::Binding {
            stored: binding.stored(),
            label: binding.label(),
        },
        Err(_) => Recorded::Pending,
    }
}

/// How stored bindings read in a chip (KEY-03, "Ctrl+Shift+O"). A binding
/// that does not parse is shown as it is stored.
#[must_use]
pub fn labels(stored: &[String]) -> Vec<String> {
    let (parsed, problems) = parse_bindings(stored);
    let mut parsed = parsed.into_iter();
    stored
        .iter()
        .map(|text| {
            if problems.iter().any(|(bad, _)| bad == text) {
                text.clone()
            } else {
                parsed
                    .next()
                    .map_or_else(|| text.clone(), |binding| binding.label())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: u32 = 0x0400_0000;
    const SHIFT: u32 = 0x0200_0000;

    fn press(key: u32, text: &str, modifiers: u32) -> QtKey {
        QtKey {
            key,
            text: text.to_owned(),
            native_scancode: 0,
            modifiers,
        }
    }

    fn binding(stored: &str, label: &str) -> Recorded {
        Recorded::Binding {
            stored: stored.to_owned(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_combination_is_stored_with_xkb_names() {
        // KEY-03
        assert_eq!(
            record(&press(0x4F, "o", CTRL | SHIFT), false),
            binding("Ctrl+Shift+o", "Ctrl+Shift+O")
        );
    }

    #[test]
    fn escape_cancels_and_backspace_clears() {
        // KEY-02
        assert_eq!(record(&press(0x0100_0000, "", 0), false), Recorded::Cancel);
        assert_eq!(record(&press(0x0100_0003, "", 0), false), Recorded::Clear);
    }

    #[test]
    fn a_modifier_alone_keeps_listening() {
        // KEY-01: modifier-only combinations cannot be recorded
        assert_eq!(
            record(&press(0x0100_0021, "", CTRL), false),
            Recorded::Pending
        );
    }

    #[test]
    fn function_keys_and_named_keys_are_recorded() {
        assert_eq!(
            record(&press(KEY_F1 + 4, "", 0), false),
            binding("F5", "F5")
        );
        assert_eq!(
            record(&press(0x0100_0004, "\r", 0), false),
            binding("Return", "Return")
        );
    }

    #[test]
    fn punctuation_comes_from_the_text() {
        assert_eq!(record(&press(0x2C, ",", 0), false), binding("comma", ","));
    }

    #[test]
    fn a_single_key_ignores_the_modifiers() {
        // SHOWN-04: "Other Key…" records any single key
        assert_eq!(record(&press(0x41, "A", SHIFT), true), binding("a", "A"));
    }

    #[test]
    fn stored_bindings_read_as_desktop_labels() {
        // KEY-03
        assert_eq!(
            labels(&["Ctrl+c".to_owned(), "Return".to_owned(), "###".to_owned()]),
            ["Ctrl+C", "Return", "###"]
        );
    }
}
