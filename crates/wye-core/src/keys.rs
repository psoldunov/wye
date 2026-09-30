//! Held modifier keys ([15-keyboard.md](../../../docs/spec/15-keyboard.md)).
//!
//! A binding is a set of modifiers. Matching is exact (KEY-05): a binding of
//! Shift fires only when Shift and no other modifier is held.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One of the four modifiers Wye reacts to. Left and right keys count the
/// same (KEY-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Super,
}

impl Modifier {
    pub const ALL: [Self; 4] = [Self::Shift, Self::Ctrl, Self::Alt, Self::Super];

    const fn bit(self) -> u8 {
        match self {
            Self::Shift => 1,
            Self::Ctrl => 2,
            Self::Alt => 4,
            Self::Super => 8,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shift => "Shift",
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Super => "Super",
        }
    }
}

impl FromStr for Modifier {
    type Err = UnknownModifier;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shift" => Ok(Self::Shift),
            "ctrl" | "control" => Ok(Self::Ctrl),
            "alt" => Ok(Self::Alt),
            "super" | "meta" | "logo" => Ok(Self::Super),
            _ => Err(UnknownModifier(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown modifier {0:?} (expected Shift, Ctrl, Alt or Super)")]
pub struct UnknownModifier(String);

/// A set of held modifiers. The empty set means "no binding".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn from_slice(modifiers: &[Modifier]) -> Self {
        let mut bits = 0;
        let mut rest = modifiers;
        while let [first, tail @ ..] = rest {
            bits |= first.bit();
            rest = tail;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, modifier: Modifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Exact match against what is held (KEY-05). An empty binding never
    /// matches, so an unset key cannot fire.
    #[must_use]
    pub const fn matches(self, held: Self) -> bool {
        !self.is_empty() && self.0 == held.0
    }

    pub fn iter(self) -> impl Iterator<Item = Modifier> {
        Modifier::ALL.into_iter().filter(move |m| self.contains(*m))
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("none");
        }
        let names: Vec<_> = self.iter().map(Modifier::name).collect();
        f.write_str(&names.join("+"))
    }
}

impl FromStr for Modifiers {
    type Err = UnknownModifier;

    /// Parses `"Shift"`, `"Ctrl+Shift"` or `"ctrl,alt"`; an empty string is
    /// the empty set.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(['+', ','])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(Modifier::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map(|mods| Self::from_slice(&mods))
    }
}

impl Serialize for Modifiers {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter().map(Modifier::name))
    }
}

impl<'de> Deserialize<'de> for Modifiers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<String>::deserialize(deserializer)?;
        names
            .iter()
            .map(|name| name.parse::<Modifier>())
            .collect::<Result<Vec<_>, _>>()
            .map(|mods| Self::from_slice(&mods))
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matching() {
        let shift = Modifiers::from_slice(&[Modifier::Shift]);
        let ctrl_shift = Modifiers::from_slice(&[Modifier::Ctrl, Modifier::Shift]);
        assert!(shift.matches(shift));
        assert!(!shift.matches(ctrl_shift));
        assert!(!ctrl_shift.matches(shift));
        assert!(!Modifiers::NONE.matches(Modifiers::NONE));
    }

    #[test]
    fn parses_and_displays() {
        let mods: Modifiers = "ctrl+Shift".parse().unwrap();
        assert_eq!(mods.to_string(), "Shift+Ctrl");
        assert_eq!("".parse::<Modifiers>().unwrap(), Modifiers::NONE);
        assert!("hyper".parse::<Modifiers>().is_err());
    }

    #[test]
    fn serde_as_name_list() {
        #[derive(Serialize, Deserialize)]
        struct H {
            k: Modifiers,
        }
        let h: H = toml::from_str(r#"k = ["Alt", "Super"]"#).unwrap();
        assert_eq!(h.k.to_string(), "Alt+Super");
        assert_eq!(
            toml::to_string(&h).unwrap().trim(),
            r#"k = ["Alt", "Super"]"#
        );
    }
}
