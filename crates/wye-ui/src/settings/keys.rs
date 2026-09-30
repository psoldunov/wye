//! The picker keys sheet's rules (KEY-20 to KEY-22, KEY-12): whether a
//! binding or a modifier set can be given to an action, and the patch that
//! gives it.
//!
//! A key belongs to one action and cannot also be a target hotkey; the three
//! held-modifier actions need different modifier sets. A clash is reported
//! with what uses the key ("Already used by …", KEY-21); the user may then
//! replace it, which takes the key from the other action or clears the
//! target's hotkey.

use serde_json::{Value, json};
use wye_core::config::PickerKeys;
use wye_core::keybinding::KeyBinding;
use wye_core::merge_patch;
use wye_core::picker::{PickerAction, PickerKeymap};
use wye_core::{Config, Modifiers};

use super::shown::{self, Entry};

/// The picker's actions with their names in the configuration (`picker.keys`).
pub const ACTION_KEYS: [(PickerAction, &str); 9] = [
    (PickerAction::Open, "open"),
    (PickerAction::Cancel, "cancel"),
    (PickerAction::Next, "next"),
    (PickerAction::Previous, "previous"),
    (PickerAction::First, "first"),
    (PickerAction::Last, "last"),
    (PickerAction::CopyLink, "copy-link"),
    (PickerAction::More, "more"),
    (PickerAction::CreateRule, "create-rule"),
];

/// The held-modifier actions: configuration key and name in the sheet
/// (KEY-13, KEY-20).
pub const MODIFIER_ACTIONS: [(&str, &str); 3] = [
    ("private-modifier", "Open in private window"),
    ("background-modifier", "Open in background"),
    ("new-window-modifier", "Open in new window"),
];

/// What already uses a binding or a modifier set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clash {
    /// Another picker action (its name).
    Action(String),
    /// A target's hotkey (the target's name).
    Target(String),
    /// Another held-modifier action (its name).
    Modifier(String),
}

impl Clash {
    /// "Already used by …" (KEY-21).
    #[must_use]
    pub fn message(&self) -> String {
        let (Self::Action(name) | Self::Target(name) | Self::Modifier(name)) = self;
        format!("Already used by “{name}”")
    }
}

/// What the sheet may do with a binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// Free: add it.
    Free,
    /// The action has it already.
    Duplicate,
    /// Something else uses it.
    Clash(Clash),
}

/// A target hotkey in use: the hotkey (canonical) and the target's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyUse {
    pub key: String,
    pub name: String,
}

fn action_of(key: &str) -> Option<PickerAction> {
    ACTION_KEYS
        .iter()
        .find(|(_, name)| *name == key)
        .map(|(action, _)| *action)
}

fn bindings_of<'a>(keys: &'a PickerKeys, key: &str) -> Option<&'a Vec<String>> {
    Some(match key {
        "open" => &keys.open,
        "cancel" => &keys.cancel,
        "next" => &keys.next,
        "previous" => &keys.previous,
        "first" => &keys.first,
        "last" => &keys.last,
        "copy-link" => &keys.copy_link,
        "more" => &keys.more,
        "create-rule" => &keys.create_rule,
        _ => return None,
    })
}

/// Parse a binding in its stored form.
///
/// # Errors
///
/// The message of the parse error.
pub fn parse(stored: &str) -> Result<KeyBinding, String> {
    stored
        .parse()
        .map_err(|error: wye_core::keybinding::BindingError| error.to_string())
}

/// May `action` (a key of `picker.keys`) take `binding`? (KEY-21, KEY-12)
///
/// `hotkeys` are the target hotkeys now set; a binding without modifiers
/// that is one of them clashes.
///
/// # Errors
///
/// A message for an action that is not one of the picker's.
pub fn check_binding(
    config: &Config,
    hotkeys: &[HotkeyUse],
    action: &str,
    binding: &KeyBinding,
) -> Result<Check, String> {
    let this = action_of(action).ok_or_else(|| format!("{action:?} is not a picker action"))?;
    let keymap = PickerKeymap::new(&config.picker.keys);
    if let Some(user) = keymap.action_using(binding) {
        return Ok(if user == this {
            Check::Duplicate
        } else {
            Check::Clash(Clash::Action(user.label().to_owned()))
        });
    }
    if binding.modifiers().is_empty()
        && let Some(used) = hotkeys.iter().find(|used| used.key == binding.key())
    {
        return Ok(Check::Clash(Clash::Target(used.name.clone())));
    }
    Ok(Check::Free)
}

/// The patch that adds `binding` to `action`. With `replace`, the action or
/// the target that had it loses it first (KEY-21).
///
/// # Errors
///
/// A message for an action that is not one of the picker's.
pub fn bind_patch(
    config: &Config,
    shown: &[Entry],
    action: &str,
    binding: &KeyBinding,
    replace: bool,
) -> Result<Value, String> {
    let keys = &config.picker.keys;
    let list =
        bindings_of(keys, action).ok_or_else(|| format!("{action:?} is not a picker action"))?;
    let mut patch = json!({});
    if replace {
        if let Some(other) = PickerKeymap::new(keys).action_using(binding) {
            let (_, other_key) = ACTION_KEYS
                .iter()
                .find(|(candidate, _)| *candidate == other)
                .ok_or_else(|| "unknown picker action".to_owned())?;
            if *other_key != action {
                let kept = without(
                    bindings_of(keys, other_key).map_or(&[][..], Vec::as_slice),
                    binding,
                );
                patch = merge(&patch, &json!({"picker": {"keys": {*other_key: kept}}}));
            }
        }
        if binding.modifiers().is_empty()
            && shown
                .iter()
                .any(|e| e.hotkey.as_deref() == Some(binding.key()))
        {
            let cleared: Vec<Entry> = shown
                .iter()
                .map(|entry| Entry {
                    hotkey: entry.hotkey.clone().filter(|key| key != binding.key()),
                    target: entry.target.clone(),
                })
                .collect();
            patch = merge(&patch, &shown::to_patch(&cleared));
        }
    }
    let mut next = list.clone();
    if !next
        .iter()
        .any(|stored| stored.parse::<KeyBinding>().is_ok_and(|b| b == *binding))
    {
        next.push(binding.stored());
    }
    Ok(merge(&patch, &json!({"picker": {"keys": {action: next}}})))
}

fn without(stored: &[String], binding: &KeyBinding) -> Vec<String> {
    stored
        .iter()
        .filter(|text| !text.parse::<KeyBinding>().is_ok_and(|b| b == *binding))
        .cloned()
        .collect()
}

fn merge(base: &Value, patch: &Value) -> Value {
    merge_patch::apply(base, patch)
}

fn modifiers_of(keys: &PickerKeys, key: &str) -> Option<Modifiers> {
    Some(match key {
        "private-modifier" => keys.private_modifier,
        "background-modifier" => keys.background_modifier,
        "new-window-modifier" => keys.new_window_modifier,
        _ => return None,
    })
}

/// May the held-modifier action `which` take `set`? The three actions need
/// different sets (KEY-21); none pressed is always free.
///
/// # Errors
///
/// A message for a key that is not a held-modifier action.
pub fn check_modifiers(
    config: &Config,
    which: &str,
    set: Modifiers,
) -> Result<Option<Clash>, String> {
    modifiers_of(&config.picker.keys, which)
        .ok_or_else(|| format!("{which:?} is not a held-modifier action"))?;
    if set.is_empty() {
        return Ok(None);
    }
    Ok(MODIFIER_ACTIONS
        .iter()
        .filter(|(key, _)| *key != which)
        .find(|(key, _)| modifiers_of(&config.picker.keys, key) == Some(set))
        .map(|(_, name)| Clash::Modifier((*name).to_owned())))
}

/// The patch that sets `which` to `set`; with `replace`, the action that had
/// the same set is cleared.
///
/// # Errors
///
/// A message for a key that is not a held-modifier action.
pub fn modifiers_patch(
    config: &Config,
    which: &str,
    set: Modifiers,
    replace: bool,
) -> Result<Value, String> {
    let value = |m: Modifiers| serde_json::to_value(m).unwrap_or(Value::Null);
    let mut keys = serde_json::Map::new();
    if replace {
        for (key, _) in MODIFIER_ACTIONS {
            if key != which
                && !set.is_empty()
                && modifiers_of(&config.picker.keys, key) == Some(set)
            {
                keys.insert(key.to_owned(), value(Modifiers::NONE));
            }
        }
    }
    modifiers_of(&config.picker.keys, which)
        .ok_or_else(|| format!("{which:?} is not a held-modifier action"))?;
    keys.insert(which.to_owned(), value(set));
    Ok(json!({"picker": {"keys": keys}}))
}

#[cfg(test)]
mod tests;
