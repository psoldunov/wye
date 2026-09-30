//! Target hotkeys (KEY-10, KEY-11, PICK-21): which key opens which tile.

use crate::config::HotkeyScheme;
use crate::keybinding::{KeyEvent, canonical_key, display_key};
use crate::target_menu::ShownTarget;

/// The digits the "Numbers" scheme assigns (KEY-10: 1–9 by position).
pub const MAX_NUMBERED: usize = 9;

/// A tile's hotkey: the canonical XKB key name and the character shown above
/// the tile (PICK-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    pub key: String,
    pub label: String,
}

impl Hotkey {
    /// `None` when `name` is not a single key.
    #[must_use]
    pub fn new(name: &str) -> Option<Self> {
        if name.contains('+') && name.trim() != "+" {
            return None;
        }
        let key = canonical_key(name).ok()?;
        Some(Self {
            label: display_key(&key),
            key,
        })
    }
}

/// One hotkey per shown target, in order, for `scheme` (KEY-10).
///
/// `reserved` holds the canonical keys the picker's actions use without a
/// modifier; no target gets one (KEY-12). Keys are unique: when two targets
/// ask for the same one, the first keeps it (SHOWN-04).
#[must_use]
pub fn assign(
    scheme: HotkeyScheme,
    shown: &[ShownTarget],
    reserved: &[String],
) -> Vec<Option<Hotkey>> {
    match scheme {
        HotkeyScheme::Off => vec![None; shown.len()],
        HotkeyScheme::Numbers => shown
            .iter()
            .enumerate()
            .map(|(i, _)| (i < MAX_NUMBERED).then(|| format!("{}", i + 1)))
            .map(|digit| digit.and_then(|d| Hotkey::new(&d)))
            .collect(),
        HotkeyScheme::PerTarget => per_target(shown, reserved),
        HotkeyScheme::Letters => letters(shown, reserved),
    }
}

fn per_target(shown: &[ShownTarget], reserved: &[String]) -> Vec<Option<Hotkey>> {
    let mut used: Vec<String> = Vec::new();
    shown
        .iter()
        .map(|target| {
            let hotkey = target.hotkey.as_deref().and_then(Hotkey::new)?;
            if reserved.contains(&hotkey.key) || used.contains(&hotkey.key) {
                return None;
            }
            used.push(hotkey.key.clone());
            Some(hotkey)
        })
        .collect()
}

/// Each target gets the first letter of its name that nobody has (KEY-10),
/// case-insensitively. A target whose letters are all taken gets none.
fn letters(shown: &[ShownTarget], reserved: &[String]) -> Vec<Option<Hotkey>> {
    let mut used: Vec<String> = reserved.to_vec();
    shown
        .iter()
        .map(|target| {
            let hotkey = target
                .info
                .long_name
                .chars()
                .filter(|c| c.is_alphabetic())
                .filter_map(|c| Hotkey::new(&c.to_string()))
                .find(|hotkey| !used.contains(&hotkey.key))?;
            used.push(hotkey.key.clone());
            Some(hotkey)
        })
        .collect()
}

/// The tile whose hotkey `event` presses (PICK-21). Matching ignores case
/// and modifiers (modifiers choose how the tile opens, KEY-13), and accepts
/// the layout's character or the Latin key at the same position (KEY-11).
#[must_use]
pub fn find(hotkeys: &[Option<Hotkey>], event: &KeyEvent) -> Option<usize> {
    hotkeys
        .iter()
        .position(|hotkey| hotkey.as_ref().is_some_and(|h| event.is_key(&h.key)))
}
