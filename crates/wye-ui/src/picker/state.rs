//! The picker's state and what each input does (PICK-20 to PICK-33,
//! KEY-13, KEY-22). Pure: every transition returns a new state and an
//! [`Effect`] the bridge carries out.

use serde::Serialize;
use wye_core::Modifiers;
use wye_core::picker::{
    KeyOutcome, OpenMode, PickerAction, TileAction, TileMenuEntry, choose, select, tile_menu,
};

use super::keys::{self, QtKey};
use super::view::{Entry, PickerView};

/// The chosen target, ready for `PickerChose` (PIPE-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOut {
    /// The target in configuration JSON; a private window is already the
    /// private target (KEY-13).
    pub target: String,
    pub background: bool,
    pub new_window: bool,
    /// The app to ask an activation token for (PICK-29), without
    /// `.desktop`; empty when unknown.
    pub app_id: String,
}

/// What the bridge does after an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The input changed only the state (or nothing) but was handled.
    None,
    /// The input is not the picker's; let it through.
    Ignored,
    /// Open the link in a target (PICK-20, PICK-21, PICK-32, PICK-33).
    Choose(ChoiceOut),
    /// Close without opening the link (PICK-23).
    Cancel,
    /// Copy the link and close (KEY-22).
    CopyLink,
    /// Close and open the rule editor pre-filled (PICK-31).
    CreateRule,
    /// Open the "⋯" menu (KEY-22 "Show more targets").
    ShowMore,
    /// Make a target the primary browser (PICK-30); the picker stays open.
    MakePrimary(String),
}

/// One entry of a tile's context menu (PICK-30), for QML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuEntry {
    /// Empty for a separator.
    pub action: String,
    pub label: String,
}

/// The picker showing one request.
#[derive(Debug, Clone)]
pub struct PickerState {
    pub view: PickerView,
    /// The selected tile (PICK-07); the first when the picker opens
    /// (PICK-24).
    pub selected: usize,
    /// Modifiers held now (KEY-13).
    pub held: Modifiers,
}

impl PickerState {
    pub fn new(view: PickerView) -> Self {
        Self {
            held: view.held,
            selected: 0,
            view,
        }
    }

    /// The way of opening the held modifiers select (KEY-13).
    pub const fn mode(&self) -> Option<OpenMode> {
        self.view.keymap.mode_for(self.held)
    }

    /// The hint line while a held-modifier action is active (PICK-14).
    pub fn hint(&self) -> &'static str {
        self.mode().map_or("", OpenMode::hint)
    }

    /// For each tile, whether it cannot open the held way (PICK-14).
    pub fn dimmed(&self) -> Vec<bool> {
        let mode = self.mode();
        self.view
            .tiles
            .iter()
            .map(|tile| mode.is_some_and(|mode| !mode.is_supported_by(&tile.info)))
            .collect()
    }

    /// The modifiers held changed.
    pub fn with_held(self, held: Modifiers) -> Self {
        Self { held, ..self }
    }

    /// The pointer is over tile `index` (PICK-22).
    pub fn hover(self, index: usize) -> Self {
        if index < self.view.tiles.len() {
            Self {
                selected: index,
                ..self
            }
        } else {
            self
        }
    }

    /// A key press (PICK-21, PICK-22, KEY-13, KEY-22).
    ///
    /// Every key event says what is held now, so it replaces the modifiers
    /// the request reported: a modifier released before the picker had the
    /// keyboard sends no release event of its own (KEY-13).
    pub fn key(self, press: &QtKey) -> (Self, Effect) {
        let state = self.with_held(keys::modifiers(press.modifiers));
        if press.is_modifier() {
            return (state, Effect::None);
        }
        state.dispatch(press)
    }

    fn dispatch(self, press: &QtKey) -> (Self, Effect) {
        let outcome = self
            .view
            .keymap
            .dispatch(&press.event(), &self.view.hotkeys());
        match outcome {
            KeyOutcome::Ignored => (self, Effect::Ignored),
            KeyOutcome::Hotkey { index, mode } => {
                let effect = self.choose(index, mode);
                (self, effect)
            }
            KeyOutcome::Action { action, mode } => self.action(action, mode),
        }
    }

    fn action(self, action: PickerAction, mode: Option<OpenMode>) -> (Self, Effect) {
        let effect = match action {
            PickerAction::Open => self.choose(self.selected, mode.or_else(|| self.mode())),
            PickerAction::Cancel => Effect::Cancel,
            PickerAction::CopyLink => Effect::CopyLink,
            PickerAction::More => Effect::ShowMore,
            PickerAction::CreateRule => Effect::CreateRule,
            PickerAction::Next
            | PickerAction::Previous
            | PickerAction::First
            | PickerAction::Last => {
                let selected = select(action, self.selected, self.view.tiles.len());
                return (Self { selected, ..self }, Effect::None);
            }
        };
        (self, effect)
    }

    /// A click on tile `index`: the middle button opens in the background
    /// (PICK-32), otherwise the held modifiers decide (PICK-20, PICK-33).
    pub fn activate(&self, index: usize, middle: bool) -> Effect {
        let mode = if middle {
            Some(OpenMode::Background)
        } else {
            self.mode()
        };
        self.choose(index, mode)
    }

    /// Open In entry `item` of group `group` (PICK-28).
    pub fn open_in(&self, group: usize, item: usize) -> Effect {
        self.view
            .overflow_entry(group, item)
            .map_or(Effect::Ignored, |entry| choice(entry, self.mode()))
    }

    /// Choose tile `index` the `mode` way.
    pub fn choose(&self, index: usize, mode: Option<OpenMode>) -> Effect {
        self.view
            .tiles
            .get(index)
            .map_or(Effect::Ignored, |entry| choice(entry, mode))
    }

    /// Tile `index`'s context menu (PICK-30).
    pub fn tile_menu(&self, index: usize) -> Vec<MenuEntry> {
        self.view.tiles.get(index).map_or_else(Vec::new, |tile| {
            tile_menu(&tile.info)
                .into_iter()
                .map(|entry| match entry {
                    TileMenuEntry::Action { action, label } => MenuEntry {
                        action: action_name(action).to_owned(),
                        label: label.to_owned(),
                    },
                    TileMenuEntry::Separator => MenuEntry {
                        action: String::new(),
                        label: String::new(),
                    },
                })
                .collect()
        })
    }

    /// A context-menu entry (PICK-30) on tile `index`.
    pub fn tile_action(&self, index: usize, action: &str) -> Effect {
        let Some(tile) = self.view.tiles.get(index) else {
            return Effect::Ignored;
        };
        match action {
            "open" => choice(tile, None),
            "open-private" => choice(tile, Some(OpenMode::Private)),
            "open-new-window" => choice(tile, Some(OpenMode::NewWindow)),
            "open-background" => choice(tile, Some(OpenMode::Background)),
            "make-primary" => Effect::MakePrimary(target_json(&tile.info.target)),
            _ => Effect::Ignored,
        }
    }
}

const fn action_name(action: TileAction) -> &'static str {
    match action {
        TileAction::Open => "open",
        TileAction::OpenPrivate => "open-private",
        TileAction::OpenNewWindow => "open-new-window",
        TileAction::OpenBackground => "open-background",
        TileAction::MakePrimary => "make-primary",
    }
}

fn choice(entry: &Entry, mode: Option<OpenMode>) -> Effect {
    let chosen = choose(&entry.info, mode).chosen;
    Effect::Choose(ChoiceOut {
        app_id: chosen
            .target
            .desktop_id()
            .map(|id| id.as_str().trim_end_matches(".desktop").to_owned())
            .unwrap_or_default(),
        target: target_json(&chosen.target),
        background: chosen.options.background,
        new_window: chosen.options.new_window,
    })
}

fn target_json(target: &wye_core::Target) -> String {
    serde_json::to_string(target).unwrap_or_else(|error| {
        tracing::warn!(%error, %target, "cannot encode a target");
        String::new()
    })
}

#[cfg(test)]
mod tests;
