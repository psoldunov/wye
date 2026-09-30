//! Picker keys (KEY-03, KEY-13, KEY-22): turning a key press into an action,
//! a hotkey choice and the held-modifier way of opening.

use super::hotkeys::{self, Hotkey};
use super::modes::{HeldActions, OpenMode};
use crate::config::PickerKeys;
use crate::keybinding::{BindingError, KeyBinding, KeyEvent, parse_bindings};
use crate::keys::Modifiers;

/// An action of the picker keys sheet (KEY-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerAction {
    Open,
    Cancel,
    Next,
    Previous,
    First,
    Last,
    CopyLink,
    More,
    CreateRule,
}

impl PickerAction {
    pub const ALL: [Self; 9] = [
        Self::Open,
        Self::Cancel,
        Self::Next,
        Self::Previous,
        Self::First,
        Self::Last,
        Self::CopyLink,
        Self::More,
        Self::CreateRule,
    ];

    /// The action's name in the sheet and in "Already used by …" (KEY-21).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open selected target",
            Self::Cancel => "Cancel",
            Self::Next => "Select next",
            Self::Previous => "Select previous",
            Self::First => "Select first",
            Self::Last => "Select last",
            Self::CopyLink => "Copy link and close",
            Self::More => "Show more targets",
            Self::CreateRule => "Create rule from link…",
        }
    }

    const fn keys(self, keys: &PickerKeys) -> &Vec<String> {
        match self {
            Self::Open => &keys.open,
            Self::Cancel => &keys.cancel,
            Self::Next => &keys.next,
            Self::Previous => &keys.previous,
            Self::First => &keys.first,
            Self::Last => &keys.last,
            Self::CopyLink => &keys.copy_link,
            Self::More => &keys.more,
            Self::CreateRule => &keys.create_rule,
        }
    }
}

/// A stored binding that could not be read and was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingProblem {
    pub action: PickerAction,
    pub text: String,
    pub error: BindingError,
}

/// What a key press does in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// One of the actions. `mode` is the held-modifier way of opening that
    /// the press also carries (KEY-13); it only matters for
    /// [`PickerAction::Open`].
    Action {
        action: PickerAction,
        mode: Option<OpenMode>,
    },
    /// A tile's hotkey: the tile's index.
    Hotkey {
        index: usize,
        mode: Option<OpenMode>,
    },
    Ignored,
}

/// The picker's keys, read from [`PickerKeys`].
#[derive(Debug, Clone)]
pub struct PickerKeymap {
    bindings: Vec<(PickerAction, Vec<KeyBinding>)>,
    held: HeldActions,
    problems: Vec<BindingProblem>,
}

impl PickerKeymap {
    /// Reads the stored keys. A binding that does not parse is dropped and
    /// listed in [`PickerKeymap::problems`]; the rest still work.
    #[must_use]
    pub fn new(keys: &PickerKeys) -> Self {
        let mut problems = Vec::new();
        let bindings = PickerAction::ALL
            .into_iter()
            .map(|action| {
                let (parsed, errors) = parse_bindings(action.keys(keys));
                problems.extend(errors.into_iter().map(|(text, error)| BindingProblem {
                    action,
                    text,
                    error,
                }));
                (action, parsed)
            })
            .collect();
        Self {
            bindings,
            held: HeldActions::from_keys(keys),
            problems,
        }
    }

    #[must_use]
    pub fn problems(&self) -> &[BindingProblem] {
        &self.problems
    }

    /// The bindings of one action, for the sheet and for tooltips.
    #[must_use]
    pub fn bindings(&self, action: PickerAction) -> &[KeyBinding] {
        self.bindings
            .iter()
            .find(|(a, _)| *a == action)
            .map_or(&[], |(_, list)| list.as_slice())
    }

    /// The way of opening for exactly these held modifiers, to show the hint
    /// line while a modifier is down (PICK-14).
    #[must_use]
    pub const fn mode_for(&self, held: Modifiers) -> Option<OpenMode> {
        self.held.mode_for(held)
    }

    /// The canonical names of the keys that some action uses without a
    /// modifier: a target hotkey cannot be one of them (KEY-12).
    #[must_use]
    pub fn plain_keys(&self) -> Vec<String> {
        self.all()
            .filter(|(_, binding)| binding.modifiers().is_empty())
            .map(|(_, binding)| binding.key().to_owned())
            .collect()
    }

    /// The action that already uses this binding (KEY-21).
    #[must_use]
    pub fn action_using(&self, binding: &KeyBinding) -> Option<PickerAction> {
        self.all()
            .find(|(_, existing)| *existing == binding)
            .map(|(action, _)| action)
    }

    /// The action that stops `key` from being a target hotkey (KEY-12).
    #[must_use]
    pub fn action_blocking_hotkey(&self, key: &str) -> Option<PickerAction> {
        let hotkey = Hotkey::new(key)?;
        self.all()
            .find(|(_, binding)| binding.modifiers().is_empty() && binding.key() == hotkey.key)
            .map(|(action, _)| action)
    }

    fn all(&self) -> impl Iterator<Item = (PickerAction, &KeyBinding)> {
        self.bindings
            .iter()
            .flat_map(|(action, list)| list.iter().map(move |binding| (*action, binding)))
    }

    fn action_for(&self, event: &KeyEvent) -> Option<PickerAction> {
        self.all()
            .find(|(_, binding)| binding.matches(event))
            .map(|(action, _)| action)
    }

    /// Decides what `event` does (PICK-21, PICK-22, KEY-13).
    ///
    /// An exact match of modifiers and key wins, so `Shift+Tab` stays "select
    /// previous" although Shift also means "private window". When nothing
    /// matches and the held modifiers are one of the three held-modifier
    /// actions, the press is tried again without them: `Shift+Return` opens
    /// the selected tile in a private window. A hotkey ignores case and works
    /// with no modifier or a held-modifier action (KEY-11).
    #[must_use]
    pub fn dispatch(&self, event: &KeyEvent, hotkeys: &[Option<Hotkey>]) -> KeyOutcome {
        if let Some(action) = self.action_for(event) {
            return KeyOutcome::Action { action, mode: None };
        }
        let mode = self.held.mode_for(event.modifiers);
        let bare = KeyEvent {
            modifiers: Modifiers::NONE,
            ..event.clone()
        };
        if !event.modifiers.is_empty() && mode.is_none() {
            return KeyOutcome::Ignored;
        }
        if let Some(action) = self.action_for(&bare) {
            return KeyOutcome::Action { action, mode };
        }
        match hotkeys::find(hotkeys, &bare) {
            Some(index) => KeyOutcome::Hotkey { index, mode },
            None => KeyOutcome::Ignored,
        }
    }
}

/// Moves the selection (PICK-22): next and previous wrap around, first and
/// last jump. Other actions leave it. `len` is the number of tiles.
#[must_use]
pub const fn select(action: PickerAction, selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    match action {
        PickerAction::Next => (selected + 1) % len,
        PickerAction::Previous => (selected + len - 1) % len,
        PickerAction::First => 0,
        PickerAction::Last => len - 1,
        _ => {
            if selected < len {
                selected
            } else {
                len - 1
            }
        }
    }
}
