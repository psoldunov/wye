//! Key bindings (KEY-03, KEY-22): parsing, storing, showing and matching
//! combinations such as `Ctrl+Shift+o`, shared by the picker and the global
//! shortcuts.
//!
//! A binding is stored with XKB key names (`"Ctrl+Shift+o"`, `"Return"`,
//! `"KP_Enter"`, `"space"`) and shown in desktop style (`Ctrl+Shift+O`).
//! Modifiers are always written in the order Ctrl, Alt, Shift, Super.
//! Matching is exact on modifiers (KEY-05) and, for the key, accepts the
//! layout's character or the Latin key at the same position (KEY-11).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::keys::{Modifier, Modifiers, UnknownModifier};

mod layout;
mod names;

pub use layout::{EVDEV_OFFSET, latin_from_evdev, latin_from_native_scancode};
pub use names::{KeyNameError, canonical_key, display_key};

/// The order modifiers are written in, stored and shown.
const WRITE_ORDER: [Modifier; 4] = [
    Modifier::Ctrl,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Super,
];

/// Why a binding could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    #[error("{0}")]
    Modifier(#[from] UnknownModifier),
    #[error("{0}")]
    Key(#[from] KeyNameError),
}

/// A modifier set plus one key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    modifiers: Modifiers,
    key: String,
}

impl KeyBinding {
    /// A binding of `key` (any spelling; it is canonicalised) with
    /// `modifiers`.
    ///
    /// # Errors
    ///
    /// Returns an error when `key` is empty, a modifier or not a key name.
    pub fn new(modifiers: Modifiers, key: &str) -> Result<Self, BindingError> {
        Ok(Self {
            modifiers,
            key: canonical_key(key)?,
        })
    }

    #[must_use]
    pub const fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// The canonical XKB key name, for example `o`, `Return` or `comma`.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The form the configuration stores (KEY-03): `Ctrl+Shift+o`.
    #[must_use]
    pub fn stored(&self) -> String {
        self.join(&self.key)
    }

    /// The form the interface shows (KEY-03): `Ctrl+Shift+O`.
    #[must_use]
    pub fn label(&self) -> String {
        self.join(&display_key(&self.key))
    }

    /// True when `event` is this binding: exactly these modifiers held, and
    /// the key (KEY-05, KEY-11).
    #[must_use]
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.modifiers == event.modifiers && event.is_key(&self.key)
    }

    fn join(&self, key: &str) -> String {
        let mut parts: Vec<&str> = WRITE_ORDER
            .iter()
            .filter(|m| self.modifiers.contains(**m))
            .map(|m| m.name())
            .collect();
        parts.push(key);
        parts.join("+")
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

impl FromStr for KeyBinding {
    type Err = BindingError;

    /// Reads `"Ctrl+Shift+o"`, `"Return"`, `"shift+tab"` or `"Ctrl++"` (the
    /// plus key). Modifier and key names are case-insensitive.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let (modifiers, key) = if let Some(head) = text.strip_suffix("++") {
            (head, "plus")
        } else if text == "+" {
            ("", "plus")
        } else {
            text.rsplit_once('+').unwrap_or(("", text))
        };
        let modifiers = modifiers
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::parse::<Modifier>)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(Modifiers::from_slice(&modifiers), key)
    }
}

impl Serialize for KeyBinding {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.stored())
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Reads a list of stored bindings (KEY-22), keeping the ones that parse and
/// reporting the rest by their text.
#[must_use]
pub fn parse_bindings(stored: &[String]) -> (Vec<KeyBinding>, Vec<(String, BindingError)>) {
    let mut bindings = Vec::with_capacity(stored.len());
    let mut errors = Vec::new();
    for text in stored {
        match text.parse() {
            Ok(binding) => bindings.push(binding),
            Err(error) => errors.push((text.clone(), error)),
        }
    }
    (bindings, errors)
}

/// A key press as the UI reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyEvent {
    /// The XKB key name the active layout gives the key (`a`, `Return`,
    /// `Cyrillic_ef`), when the toolkit reports one.
    pub key: Option<String>,
    /// The text the key produces (`a`, `ф`), when any.
    pub text: Option<String>,
    /// Qt's `nativeScanCode`: the physical key, evdev code plus 8.
    pub native_scancode: Option<u32>,
    /// The modifiers held, left and right alike (KEY-01).
    pub modifiers: Modifiers,
}

impl KeyEvent {
    /// An event with a key name and modifiers.
    #[must_use]
    pub fn named(key: &str, modifiers: Modifiers) -> Self {
        Self {
            key: Some(key.to_owned()),
            modifiers,
            ..Self::default()
        }
    }

    /// True when the event is the key `canonical` (an already canonical key
    /// name): by name, by the text it produces, or by the Latin key at its
    /// physical position (KEY-11).
    #[must_use]
    pub fn is_key(&self, canonical: &str) -> bool {
        let same = |candidate: &str| canonical_key(candidate).is_ok_and(|key| key == canonical);
        self.key.as_deref().is_some_and(same)
            || self.text.as_deref().is_some_and(same)
            || self
                .native_scancode
                .and_then(latin_from_native_scancode)
                .is_some_and(|latin| same(&latin.to_string()))
    }
}

#[cfg(test)]
mod tests;
