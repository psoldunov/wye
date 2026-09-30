//! The JSON the picker's QML draws from: tiles (PICK-04 to PICK-07,
//! PICK-14) and the flattened Open In list (PICK-08, PICK-28).

use serde::Serialize;
use wye_api::Badge;

use super::state::PickerState;
use super::view::Entry;

/// One tile as QML draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileJson {
    pub name: String,
    pub icon: String,
    /// The hotkey label above the icon; empty for none (PICK-04).
    pub hotkey: String,
    #[serde(flatten)]
    pub badge: BadgeJson,
    /// Cannot open the held way (PICK-14).
    pub dimmed: bool,
}

/// A profile badge (PICK-06): a picture, or an initial on a colour.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeJson {
    #[serde(rename = "badgeImage")]
    pub image: String,
    #[serde(rename = "badgeInitial")]
    pub initial: String,
    #[serde(rename = "badgeColor")]
    pub color: String,
}

/// One row of the Open In submenu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInJson {
    /// `header` or `item`.
    pub kind: &'static str,
    pub label: String,
    pub icon: String,
    pub group: usize,
    pub item: usize,
}

/// The tiles of `state`.
pub fn tiles(state: &PickerState) -> Vec<TileJson> {
    state
        .view
        .tiles
        .iter()
        .zip(state.dimmed())
        .map(|(tile, dimmed)| TileJson {
            name: tile.name.clone(),
            icon: icon(tile),
            hotkey: tile
                .hotkey
                .as_ref()
                .map(|hotkey| hotkey.label.clone())
                .unwrap_or_default(),
            badge: badge(tile.badge.as_ref()),
            dimmed,
        })
        .collect()
}

/// The Open In list: each group's label as a header row (when it has one),
/// then its targets.
pub fn open_in(state: &PickerState) -> Vec<OpenInJson> {
    state
        .view
        .overflow
        .iter()
        .enumerate()
        .flat_map(|(group_index, group)| {
            let header = (!group.label.is_empty()).then(|| OpenInJson {
                kind: "header",
                label: group.label.clone(),
                icon: String::new(),
                group: group_index,
                item: 0,
            });
            let items = group
                .entries
                .iter()
                .enumerate()
                .map(move |(item, entry)| OpenInJson {
                    kind: "item",
                    label: entry.name.clone(),
                    icon: icon(entry),
                    group: group_index,
                    item,
                });
            header.into_iter().chain(items)
        })
        .collect()
}

fn icon(entry: &Entry) -> String {
    entry.info.icon.clone().unwrap_or_default()
}

fn badge(badge: Option<&Badge>) -> BadgeJson {
    match badge {
        Some(Badge::Image { image }) => BadgeJson {
            image: image.clone(),
            ..BadgeJson::default()
        },
        Some(Badge::Initial { initial, color }) => BadgeJson {
            initial: initial.clone(),
            color: color.clone(),
            ..BadgeJson::default()
        },
        None => BadgeJson::default(),
    }
}

/// `value` as JSON text for a QML property; `[]` if it cannot be encoded.
pub fn text<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot encode picker data for QML");
        "[]".to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    #[test]
    fn tiles_carry_hotkey_labels_badges_and_dimming() {
        // PICK-04, PICK-06, PICK-14.
        let state = PickerState::new(fixture::view());
        let json = serde_json::to_value(tiles(&state)).expect("json");
        assert_eq!(json[0]["hotkey"], "F");
        assert_eq!(json[1]["badgeInitial"], "W");
        assert_eq!(json[1]["badgeColor"], "#336699");
        assert_eq!(json[0]["dimmed"], false);
    }

    #[test]
    fn open_in_lists_a_header_then_its_targets() {
        // PICK-28.
        let state = PickerState::new(fixture::view());
        let rows = open_in(&state);
        assert_eq!(rows[0].kind, "header");
        assert_eq!(rows[0].label, "Browsers");
        assert_eq!((rows[1].kind, rows[1].group, rows[1].item), ("item", 0, 0));
        assert_eq!(rows[1].label, "Brave");
    }
}
