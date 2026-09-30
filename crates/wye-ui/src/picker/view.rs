//! A `PickerRequest` read into what the picker needs: tiles with the core's
//! target info (for held-modifier choices and tile menus), the keymap, the
//! metrics and the URL line (PICK-03 to PICK-14, KEY-10 to KEY-22).

use serde::Serialize;
use url::Url;
use wye_api::picker::{
    IconSize as WireIconSize, OverflowGroup, PickerKeys as WireKeys, PickerRequest, PickerTile,
    Placement,
};
use wye_api::{Badge as WireBadge, TargetCapabilities};
use wye_core::Target;
use wye_core::config::{IconSize, PickerKeys};
use wye_core::link_text::LinkParts;
use wye_core::picker::{Hotkey, PickerKeymap, TILES_PER_ROW, TileMetrics};
use wye_core::target_menu::{TargetCaps, TargetInfo};

/// Longest the URL line may be, in characters, before it is cut in the
/// middle (PICK-09).
pub const URL_LINE_CHARS: usize = 64;

/// One target the picker offers: a tile or an Open In entry.
#[derive(Debug, Clone)]
pub struct Entry {
    pub info: TargetInfo,
    /// Shown under the icon.
    pub name: String,
    pub hotkey: Option<Hotkey>,
    pub badge: Option<WireBadge>,
}

/// A group of the "⋯" menu's Open In submenu (PICK-28).
#[derive(Debug, Clone)]
pub struct Group {
    pub label: String,
    pub entries: Vec<Entry>,
}

/// The URL line (PICK-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrlLine {
    pub host: String,
    pub rest: String,
    pub full: String,
    pub source_name: String,
    pub source_icon: String,
}

/// Everything the picker shows for one request.
#[derive(Debug, Clone)]
pub struct PickerView {
    pub tiles: Vec<Entry>,
    pub overflow: Vec<Group>,
    pub keymap: PickerKeymap,
    pub metrics: TileMetrics,
    pub show_names: bool,
    pub show_url: bool,
    pub url: UrlLine,
    /// Modifiers held as the picker opens (KEY-13).
    pub held: wye_core::Modifiers,
    pub placement: Option<Placement>,
    /// PKS-06: choosing opens nothing.
    pub preview: bool,
}

/// Why a request could not be shown.
#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("the picker request is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the picker keys are not valid: {0}")]
    Keys(serde_json::Error),
}

impl PickerView {
    /// Read `json`, a [`PickerRequest`]. Tiles whose target cannot be read
    /// are left out and logged.
    ///
    /// # Errors
    ///
    /// When the text is not a request or its keys cannot be read.
    pub fn parse(json: &str) -> Result<Self, ViewError> {
        let request: PickerRequest = serde_json::from_str(json)?;
        let keys = picker_keys(&request.keys).map_err(ViewError::Keys)?;
        Ok(Self {
            tiles: entries(&request.tiles),
            overflow: request.overflow.iter().filter_map(group).collect(),
            keymap: PickerKeymap::new(&keys),
            metrics: TileMetrics::for_size(icon_size(request.settings.icon_size)),
            show_names: request.settings.show_names,
            show_url: request.settings.show_url,
            url: url_line(&request),
            held: held(&request),
            placement: request.placement.clone(),
            preview: request.preview,
        })
    }

    /// The hotkeys in tile order, for the keymap.
    pub fn hotkeys(&self) -> Vec<Option<Hotkey>> {
        self.tiles.iter().map(|tile| tile.hotkey.clone()).collect()
    }

    /// Tiles per row (PICK-13).
    pub fn columns(&self) -> usize {
        self.tiles.len().clamp(1, TILES_PER_ROW)
    }

    /// The Open In entry `item` of group `group`.
    pub fn overflow_entry(&self, group: usize, item: usize) -> Option<&Entry> {
        self.overflow.get(group)?.entries.get(item)
    }
}

fn entries(tiles: &[PickerTile]) -> Vec<Entry> {
    tiles.iter().filter_map(entry).collect()
}

fn entry(tile: &PickerTile) -> Option<Entry> {
    let target: Target = serde_json::from_value(tile.target.clone())
        .inspect_err(|error| tracing::warn!(%error, name = tile.name, "skipping a picker tile"))
        .ok()?;
    Some(Entry {
        info: TargetInfo {
            target,
            name: tile.name.clone(),
            long_name: tile.name.clone(),
            icon: tile.icon.clone(),
            badge: None,
            caps: caps(tile.capabilities),
        },
        name: tile.name.clone(),
        hotkey: tile.hotkey.as_deref().and_then(Hotkey::new),
        badge: tile.badge.clone(),
    })
}

const fn caps(capabilities: TargetCapabilities) -> TargetCaps {
    TargetCaps {
        private: capabilities.private,
        new_window: capabilities.new_window,
    }
}

fn group(group: &OverflowGroup) -> Option<Group> {
    let entries: Vec<Entry> = entries(&group.tiles)
        .into_iter()
        .map(|entry| Entry {
            hotkey: None,
            ..entry
        })
        .collect();
    (!entries.is_empty()).then(|| Group {
        label: group.label.clone(),
        entries,
    })
}

/// The wire keys in the configuration's shape: `actions` under their own
/// names, `modifier_actions` as `<name>-modifier`. Missing actions keep
/// their defaults.
fn picker_keys(keys: &WireKeys) -> Result<PickerKeys, serde_json::Error> {
    let actions = keys
        .actions
        .iter()
        .map(|(name, bindings)| (name.clone(), serde_json::json!(bindings)));
    let modifiers = keys
        .modifier_actions
        .iter()
        .map(|(name, held)| (format!("{name}-modifier"), serde_json::json!(held)));
    serde_json::from_value(serde_json::Value::Object(
        actions.chain(modifiers).collect(),
    ))
}

const fn icon_size(size: WireIconSize) -> IconSize {
    match size {
        WireIconSize::Small => IconSize::Small,
        WireIconSize::Medium => IconSize::Medium,
        WireIconSize::Large => IconSize::Large,
    }
}

fn url_line(request: &PickerRequest) -> UrlLine {
    let parts = Url::parse(&request.url.full).map_or_else(
        |_| LinkParts {
            host: request.url.host.clone(),
            rest: request.url.rest.clone(),
        },
        |url| LinkParts::new(&url),
    );
    let short = parts.truncated(URL_LINE_CHARS);
    let source = request.source.as_ref();
    UrlLine {
        host: short.host,
        rest: short.rest,
        full: request.url.full.clone(),
        source_name: source.map(|s| s.name.clone()).unwrap_or_default(),
        source_icon: source.and_then(|s| s.icon.clone()).unwrap_or_default(),
    }
}

fn held(request: &PickerRequest) -> wye_core::Modifiers {
    let held: Vec<wye_core::Modifier> = request
        .held
        .iter()
        .filter_map(|modifier| modifier.as_str().parse().ok())
        .collect();
    wye_core::Modifiers::from_slice(&held)
}

#[cfg(test)]
mod tests;
