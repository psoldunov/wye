//! Held picker modifiers (KEY-13, PICK-14, PICK-30, PICK-32, PICK-33): how the
//! chosen target opens.

use crate::config::PickerKeys;
use crate::keys::Modifiers;
use crate::pipeline::{Chosen, OpenOptions};
use crate::target::Target;
use crate::target_menu::{TargetCaps, TargetInfo};

/// A way to open the chosen target that a held modifier selects (KEY-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenMode {
    Private,
    Background,
    NewWindow,
}

impl OpenMode {
    pub const ALL: [Self; 3] = [Self::Private, Self::Background, Self::NewWindow];

    /// The hint line shown while the modifier is held (PICK-14).
    #[must_use]
    pub const fn hint(self) -> &'static str {
        match self {
            Self::Private => "Open in a private window",
            Self::Background => "Open in the background",
            Self::NewWindow => "Open in a new window",
        }
    }

    /// Whether `info` can open this way. Opening in the background is best
    /// effort for every target (LAUNCH-04).
    #[must_use]
    pub fn is_supported_by(self, info: &TargetInfo) -> bool {
        match self {
            Self::Private => info.caps.private,
            Self::Background => true,
            Self::NewWindow => info.caps.new_window,
        }
    }
}

/// The three held-modifier actions of the picker keys sheet (KEY-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldActions {
    private: Modifiers,
    background: Modifiers,
    new_window: Modifiers,
}

impl HeldActions {
    #[must_use]
    pub const fn from_keys(keys: &PickerKeys) -> Self {
        Self {
            private: keys.private_modifier,
            background: keys.background_modifier,
            new_window: keys.new_window_modifier,
        }
    }

    /// The action for exactly these held modifiers (KEY-05). Nothing held, or
    /// a set no action uses, is `None`. When the user gave two actions the
    /// same set (KEY-21 warns about it), private wins over background over
    /// new window.
    #[must_use]
    pub const fn mode_for(&self, held: Modifiers) -> Option<OpenMode> {
        if self.private.matches(held) {
            Some(OpenMode::Private)
        } else if self.background.matches(held) {
            Some(OpenMode::Background)
        } else if self.new_window.matches(held) {
            Some(OpenMode::NewWindow)
        } else {
            None
        }
    }
}

/// The result of choosing a target with a held modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub chosen: Chosen,
    /// False when the target cannot open the requested way and opens
    /// normally (PICK-14 dims such tiles).
    pub honoured: bool,
}

/// Applies `mode` to the chosen target (KEY-13, PICK-33): a private window
/// becomes the browser's private target, the other two are open options.
#[must_use]
pub fn choose(info: &TargetInfo, mode: Option<OpenMode>) -> Choice {
    let plain = Chosen {
        target: info.target.clone(),
        options: OpenOptions::default(),
    };
    let Some(mode) = mode else {
        return Choice {
            chosen: plain,
            honoured: true,
        };
    };
    if !mode.is_supported_by(info) {
        return Choice {
            chosen: plain,
            honoured: false,
        };
    }
    let chosen = match (mode, &info.target) {
        (OpenMode::Private, Target::App(app)) => Chosen {
            target: Target::Private(app.clone()),
            ..plain
        },
        (OpenMode::Background, _) => Chosen {
            options: OpenOptions {
                background: true,
                ..plain.options
            },
            ..plain
        },
        (OpenMode::NewWindow, _) => Chosen {
            options: OpenOptions {
                new_window: true,
                ..plain.options
            },
            ..plain
        },
        // Private is already private; nothing else has a private window.
        (OpenMode::Private, _) => plain,
    };
    Choice {
        chosen,
        honoured: true,
    }
}

/// What the tile's context menu offers (PICK-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileAction {
    Open,
    OpenPrivate,
    OpenNewWindow,
    OpenBackground,
    MakePrimary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileMenuEntry {
    Action {
        action: TileAction,
        label: &'static str,
    },
    Separator,
}

/// The context menu of a tile (PICK-30): each open variant only when the
/// target supports it, then "Make Primary Browser".
#[must_use]
pub fn tile_menu(info: &TargetInfo) -> Vec<TileMenuEntry> {
    let TargetCaps {
        private,
        new_window,
    } = info.caps;
    // A private target already is the private window.
    let offer_private = private && !matches!(info.target, Target::Private(_));
    let variants = [
        (
            TileAction::OpenPrivate,
            "Open in Private Window",
            offer_private,
        ),
        (TileAction::OpenNewWindow, "Open in New Window", new_window),
        (TileAction::OpenBackground, "Open in Background", true),
    ];
    let mut entries = vec![TileMenuEntry::Action {
        action: TileAction::Open,
        label: "Open",
    }];
    entries.extend(
        variants
            .into_iter()
            .filter(|&(_, _, offered)| offered)
            .map(|(action, label, _)| TileMenuEntry::Action { action, label }),
    );
    entries.push(TileMenuEntry::Separator);
    entries.push(TileMenuEntry::Action {
        action: TileAction::MakePrimary,
        label: "Make Primary Browser",
    });
    entries
}
