//! The `PickerRequest` the UI draws, built from the core picker model
//! (PICK-03 to PICK-14, KEY-10 to KEY-13, KEY-22). Pure: no IO.
//!
//! The service decides tiles, hotkeys, grouping and key bindings; the UI
//! only renders them and sends the choice back.

use url::Url;
use wye_api::picker::{
    ACTIONS, IconSize as WireIconSize, MODIFIER_ACTIONS, OverflowGroup, PickerKeys, PickerRequest,
    PickerSettings, PickerSource, PickerTile, PickerUrl, Placement,
};
use wye_api::{Badge as WireBadge, TargetCapabilities};
use wye_core::config::{Config, IconSize, PickerKeys as CoreKeys};
use wye_core::link_text::LinkParts;
use wye_core::picker::{OverflowEntry, PickerModel, SourceLabel, Tile, choosable};
use wye_core::target_menu::{Badge, MenuChoice, MenuSection, TargetCatalog, TargetInfo};
use wye_core::{Modifiers, Target};

/// Everything one request is built from.
pub(crate) struct Input<'a> {
    pub config: &'a Config,
    pub catalog: &'a TargetCatalog,
    /// The link as it will open (cleaned and expanded).
    pub url: &'a Url,
    pub source: Option<SourceLabel>,
    /// Modifiers already held when the picker opens (KEY-13).
    pub held: Modifiers,
    /// The pointer; centred on the active output when `None` (PICK-02).
    pub placement: Option<Placement>,
    /// PKS-06: choosing opens nothing.
    pub preview: bool,
    /// The link's activation token, for the picker window's focus.
    pub activation_token: Option<String>,
}

/// Every target `PickerChose` may answer `input`'s request with: each tile
/// and **Open In** entry as it is, and each way it can open (its private
/// target, KEY-13), as the core's own choice logic produces them.
pub(crate) fn offered(input: &Input<'_>) -> Vec<Target> {
    let model = PickerModel::new(
        input.config,
        input.catalog,
        Some(input.url),
        input.source.clone(),
    );
    let tiles = model.tiles.iter().flat_map(|tile| choosable(&tile.info));
    let open_in = model
        .overflow
        .iter()
        .filter_map(|entry| match entry {
            OverflowEntry::OpenIn(menu) => Some(menu),
            OverflowEntry::Separator | OverflowEntry::Action(_) => None,
        })
        .flat_map(|menu| menu.sections.iter())
        .flat_map(|section| section.items.iter())
        .filter_map(|item| match &item.choice {
            MenuChoice::Target(target) => Some(target),
            MenuChoice::Other => None,
        })
        .flat_map(|target| {
            input
                .catalog
                .describe(target)
                .map_or_else(|| vec![target.clone()], |info| choosable(&info))
        });
    tiles
        .chain(open_in)
        .fold(Vec::new(), |mut offered, target| {
            if !offered.contains(&target) {
                offered.push(target);
            }
            offered
        })
}

/// The request for `input`.
pub(crate) fn build(input: &Input<'_>) -> PickerRequest {
    let model = PickerModel::new(
        input.config,
        input.catalog,
        Some(input.url),
        input.source.clone(),
    );
    let settings = &input.config.picker;
    PickerRequest {
        url: url(input.url),
        source: input.source.as_ref().map(|source| PickerSource {
            name: source.name.clone(),
            icon: source.icon.clone(),
        }),
        tiles: model.tiles.iter().map(tile).collect(),
        overflow: overflow(&model.overflow, input.catalog),
        settings: PickerSettings {
            icon_size: icon_size(settings.icon_size),
            show_names: settings.show_names,
            show_url: settings.show_url,
            show_badge: settings.show_profile_badge,
        },
        keys: keys(&settings.keys),
        held: modifiers(input.held),
        placement: input.placement.clone(),
        preview: input.preview,
        activation_token: input.activation_token.clone(),
    }
}

fn url(link: &Url) -> PickerUrl {
    let parts = LinkParts::new(link);
    PickerUrl {
        full: link.to_string(),
        host: parts.host,
        rest: parts.rest,
    }
}

fn tile(tile: &Tile) -> PickerTile {
    PickerTile {
        hotkey: tile.hotkey.as_ref().map(|hotkey| hotkey.key.clone()),
        badge: tile.badge.as_ref().map(badge),
        name: tile.name.clone(),
        ..describe(&tile.info)
    }
}

/// A tile for `info`, without hotkey.
fn describe(info: &TargetInfo) -> PickerTile {
    PickerTile {
        target: target(&info.target),
        name: info.long_name.clone(),
        icon: info.icon.clone(),
        badge: info.badge.as_ref().map(badge),
        hotkey: None,
        capabilities: TargetCapabilities {
            private: info.caps.private,
            new_window: info.caps.new_window,
            // Best effort for every target (LAUNCH-04).
            background: true,
        },
    }
}

/// A target in its configuration JSON shape, which `PickerChose` sends back.
pub(crate) fn target(target: &Target) -> serde_json::Value {
    serde_json::to_value(target).unwrap_or_else(|error| {
        // Target's serialisation cannot fail; keep the request whole anyway.
        tracing::warn!(%error, %target, "cannot encode a picker target");
        serde_json::Value::Null
    })
}

fn badge(badge: &Badge) -> WireBadge {
    match badge {
        Badge::Image(path) => WireBadge::Image {
            image: path.clone(),
        },
        Badge::Initial { text, color } => WireBadge::Initial {
            initial: text.clone(),
            color: format!("#{color:06x}"),
        },
    }
}

/// PICK-08, PICK-28: the "⋯" menu's **Open In** groups. "Other…" (the app
/// chooser) is not offered from the picker.
fn overflow(entries: &[OverflowEntry], catalog: &TargetCatalog) -> Vec<OverflowGroup> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            OverflowEntry::OpenIn(menu) => Some(menu),
            OverflowEntry::Separator | OverflowEntry::Action(_) => None,
        })
        .flat_map(|menu| menu.sections.iter())
        .filter_map(|section| group(section, catalog))
        .collect()
}

fn group(section: &MenuSection, catalog: &TargetCatalog) -> Option<OverflowGroup> {
    let tiles: Vec<PickerTile> = section
        .items
        .iter()
        .filter_map(|item| match &item.choice {
            MenuChoice::Target(target) => Some((target, item)),
            MenuChoice::Other => None,
        })
        .map(|(target, item)| {
            let described = catalog.describe(target).map_or_else(
                || PickerTile {
                    target: self::target(target),
                    ..PickerTile::default()
                },
                |info| describe(&info),
            );
            PickerTile {
                name: item.label.clone(),
                icon: item.icon.clone(),
                badge: item.badge.as_ref().map(badge),
                ..described
            }
        })
        .collect();
    if tiles.is_empty() {
        return None;
    }
    Some(OverflowGroup {
        label: section.header.clone().unwrap_or_default(),
        icon: None,
        tiles,
    })
}

const fn icon_size(size: IconSize) -> WireIconSize {
    match size {
        IconSize::Small => WireIconSize::Small,
        IconSize::Medium => WireIconSize::Medium,
        IconSize::Large => WireIconSize::Large,
    }
}

/// KEY-22: the bindings under the names of [`ACTIONS`] and
/// [`MODIFIER_ACTIONS`], read through the configuration's own serialisation
/// so both stay in step with `config.toml`.
fn keys(keys: &CoreKeys) -> PickerKeys {
    let table = serde_json::to_value(keys).unwrap_or_default();
    let strings = |name: &str| -> Vec<String> {
        table
            .get(name)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    PickerKeys {
        actions: ACTIONS
            .iter()
            .map(|&name| (name.to_owned(), strings(name)))
            .collect(),
        modifier_actions: MODIFIER_ACTIONS
            .iter()
            .map(|&name| {
                let held = strings(&format!("{name}-modifier"))
                    .iter()
                    .filter_map(|modifier| modifier.parse().ok())
                    .collect();
                (name.to_owned(), held)
            })
            .collect(),
    }
}

fn modifiers(held: Modifiers) -> Vec<wye_api::context::Modifier> {
    held.iter()
        .filter_map(|modifier| modifier.name().parse().ok())
        .collect()
}

#[cfg(test)]
mod tests;
