//! The picker's model (PICK-03 to PICK-14, PICK-21 to PICK-33, KEY-10 to
//! KEY-13, KEY-22): what to draw and what each key does, from the
//! configuration and the discovered targets. The interface only renders it
//! and forwards input.

use std::ops::Range;

use serde::Serialize;
use url::Url;

use crate::config::{Config, IconSize};
use crate::link_text::LinkParts;
use crate::target::Target;
use crate::target_menu::{
    Badge, MenuSpec, ShownTarget, Surface, TargetCatalog, TargetInfo, TargetMenu, shown_targets,
};

mod hotkeys;
mod keymap;
mod modes;

pub use hotkeys::{Hotkey, MAX_NUMBERED, assign as assign_hotkeys, find as find_hotkey};
pub use keymap::{BindingProblem, KeyOutcome, PickerAction, PickerKeymap, select};
pub use modes::{Choice, HeldActions, OpenMode, TileAction, TileMenuEntry, choose, tile_menu};

/// Tiles per row before the row wraps (PICK-13).
pub const TILES_PER_ROW: usize = 8;

/// Pixel sizes for an icon size (the metrics table of 02-picker.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TileMetrics {
    pub icon: u16,
    pub pitch: u16,
    pub badge: u16,
}

impl TileMetrics {
    /// PICK-11.
    #[must_use]
    pub const fn for_size(size: IconSize) -> Self {
        match size {
            IconSize::Small => Self {
                icon: 32,
                pitch: 48,
                badge: 18,
            },
            IconSize::Medium => Self {
                icon: 42,
                pitch: 56,
                badge: 24,
            },
            IconSize::Large => Self {
                icon: 52,
                pitch: 64,
                badge: 32,
            },
        }
    }
}

/// One tile (PICK-04): hotkey, icon, name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub info: TargetInfo,
    /// The name under the icon. The interface elides it (PICK-05) and hides
    /// it when [`PickerModel::show_names`] is off.
    pub name: String,
    pub hotkey: Option<Hotkey>,
    /// The profile badge, only when PKS-04 is on.
    pub badge: Option<Badge>,
}

/// An entry of the "⋯" menu's own actions (PICK-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverflowAction {
    CopyLink,
    CreateRule,
    Settings,
}

impl OverflowAction {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CopyLink => "Copy Link",
            Self::CreateRule => "Create Rule…",
            Self::Settings => "Settings…",
        }
    }
}

/// The "⋯" menu, top to bottom (PICK-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverflowEntry {
    /// **Open In** ›, with every known target that is not a tile (PICK-28),
    /// grouped like the target menu.
    OpenIn(TargetMenu),
    Separator,
    Action(OverflowAction),
}

/// The source app shown in the URL line (PICK-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLabel {
    pub name: String,
    pub icon: Option<String>,
}

/// The line under the tiles (PICK-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlLine {
    /// "from Slack", when the source is known.
    pub source: Option<SourceLabel>,
    /// The link, host apart from the rest so the host can be emphasised.
    pub link: LinkParts,
    /// The full link, for the tooltip.
    pub full: String,
}

impl UrlLine {
    /// The link cut in the middle to `max` characters, the host kept
    /// (PICK-09).
    #[must_use]
    pub fn truncated(&self, max: usize) -> LinkParts {
        self.link.truncated(max)
    }
}

/// Everything the picker draws.
#[derive(Debug, Clone)]
pub struct PickerModel {
    pub tiles: Vec<Tile>,
    /// The tiles of each row, as index ranges (PICK-13).
    pub rows: Vec<Range<usize>>,
    /// The tile selected when the picker opens (PICK-24).
    pub selected: usize,
    pub metrics: TileMetrics,
    /// PKS-02.
    pub show_names: bool,
    pub overflow: Vec<OverflowEntry>,
    pub url_line: Option<UrlLine>,
}

impl PickerModel {
    /// Builds the picker for a link (PICK-03 to PICK-13).
    ///
    /// `link` and `source` feed the URL line, which shows only when PKS-03 is
    /// on.
    #[must_use]
    pub fn new(
        config: &Config,
        catalog: &TargetCatalog,
        link: Option<&Url>,
        source: Option<SourceLabel>,
    ) -> Self {
        let settings = &config.picker;
        let shown = shown_targets(config, catalog);
        let keymap = PickerKeymap::new(&settings.keys);
        let hotkeys = assign_hotkeys(settings.hotkeys, &shown, &keymap.plain_keys());
        let tiles: Vec<Tile> = shown
            .into_iter()
            .zip(hotkeys)
            .map(|(target, hotkey)| tile(target, hotkey, settings.show_profile_badge))
            .collect();
        let url_line = link.filter(|_| settings.show_url).map(|url| UrlLine {
            source,
            link: LinkParts::new(url),
            full: url.to_string(),
        });
        Self {
            rows: rows(tiles.len()),
            selected: 0,
            metrics: TileMetrics::for_size(settings.icon_size),
            show_names: settings.show_names,
            overflow: overflow(&tiles, catalog),
            url_line,
            tiles,
        }
    }

    /// The tile a hotkey press or a click on tile `index` refers to.
    #[must_use]
    pub fn tile(&self, index: usize) -> Option<&Tile> {
        self.tiles.get(index)
    }

    /// For each tile, whether it cannot open the way the held modifiers ask
    /// for and is dimmed (PICK-14). Nothing is dimmed without a mode.
    #[must_use]
    pub fn dimmed(&self, mode: Option<OpenMode>) -> Vec<bool> {
        self.tiles
            .iter()
            .map(|tile| mode.is_some_and(|m| !m.is_supported_by(&tile.info)))
            .collect()
    }

    /// The hotkeys in tile order, for [`PickerKeymap::dispatch`].
    #[must_use]
    pub fn hotkeys(&self) -> Vec<Option<Hotkey>> {
        self.tiles.iter().map(|tile| tile.hotkey.clone()).collect()
    }

    /// Chooses tile `index` with the held-modifier way of opening `mode`
    /// (PICK-20, PICK-21, PICK-32, PICK-33).
    #[must_use]
    pub fn choose(&self, index: usize, mode: Option<OpenMode>) -> Option<Choice> {
        self.tile(index).map(|tile| choose(&tile.info, mode))
    }

    /// Whether the picker has nothing to show but the "⋯" menu.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

fn tile(shown: ShownTarget, hotkey: Option<Hotkey>, show_badge: bool) -> Tile {
    Tile {
        name: shown.info.long_name.clone(),
        badge: shown.info.badge.clone().filter(|_| show_badge),
        info: shown.info,
        hotkey,
    }
}

/// Splits `count` tiles into rows of at most [`TILES_PER_ROW`] (PICK-13).
#[must_use]
pub fn rows(count: usize) -> Vec<Range<usize>> {
    (0..count)
        .step_by(TILES_PER_ROW)
        .map(|start| start..(start + TILES_PER_ROW).min(count))
        .collect()
}

// PICK-08 and PICK-28
fn overflow(tiles: &[Tile], catalog: &TargetCatalog) -> Vec<OverflowEntry> {
    let on_tiles: Vec<Target> = tiles.iter().map(|t| t.info.target.clone()).collect();
    let open_in = TargetMenu::build(
        &MenuSpec {
            surface: Surface::Picker,
            current: None,
            primary: &Target::Picker,
            own_apps: &[],
            exclude: &on_tiles,
        },
        catalog,
    );
    vec![
        OverflowEntry::OpenIn(open_in),
        OverflowEntry::Separator,
        OverflowEntry::Action(OverflowAction::CopyLink),
        OverflowEntry::Action(OverflowAction::CreateRule),
        OverflowEntry::Separator,
        OverflowEntry::Action(OverflowAction::Settings),
    ]
}

#[cfg(test)]
mod tests;
