//! String enums of the wire format.
//!
//! Many arguments are plain strings on the bus (`entry`, `force`, a window
//! name, an action name). [`wire_enum!`] declares each set once, with the exact
//! wire spelling, and derives parsing, printing and serde from it so the two
//! directions cannot drift apart.

use std::fmt;

/// A string the contract does not define for this argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownValue {
    /// What the string was meant to name, for example `"window"`.
    pub kind: &'static str,
    /// The string that was received.
    pub value: String,
}

impl fmt::Display for UnknownValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown {} `{}`", self.kind, self.value)
    }
}

impl std::error::Error for UnknownValue {}

/// Declare an enum whose variants travel as fixed strings.
///
/// `enum Name as "kind" { Variant = "wire", … }` derives `as_str`, `ALL`,
/// `Display`, `FromStr` (failing with [`UnknownValue`]) and serde with the
/// same spellings.
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident as $kind:literal {
            $( $(#[$vmeta:meta])* $variant:ident = $wire:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        $vis enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire spelling.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::wire::UnknownValue;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err($crate::wire::UnknownValue {
                        kind: $kind,
                        value: value.to_owned(),
                    }),
                }
            }
        }
    };
}
