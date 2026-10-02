//! The shown browsers sheet (SHOWN-01 to SHOWN-08): the rows it lists and
//! every edit as a pure function from the shown list to the next one.
//!
//! The shown list is `browsers.shown` in the configuration: ordered
//! entries of a target and an optional hotkey. Checked rows are those
//! entries, in that order (SHOWN-03); unchecked rows are every other
//! candidate, which have no place in the list and so no hotkey. Choosing a
//! hotkey for an unchecked row therefore also checks it.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::targets::{TargetInfo, TargetInventory, TargetKind};

use super::menu;

/// One entry of `browsers.shown`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub target: Value,
    pub hotkey: Option<String>,
}

impl Entry {
    fn to_value(&self) -> Value {
        match &self.hotkey {
            Some(hotkey) => json!({"target": self.target, "hotkey": hotkey}),
            None => json!({"target": self.target}),
        }
    }
}

/// `browsers.shown` of a configuration; entries that are not a target and
/// an optional hotkey are skipped.
#[must_use]
pub fn entries(config: &Value) -> Vec<Entry> {
    config
        .pointer("/browsers/shown")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|entry| {
                    Some(Entry {
                        target: entry.get("target")?.clone(),
                        hotkey: entry
                            .get("hotkey")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The list the picker actually shows: what the user chose, or, when they
/// chose nothing, the installed browsers alphabetically, so the picker is
/// never empty (the core's `shown_targets`).
#[must_use]
pub fn effective(
    inventory: &TargetInventory,
    stored: &[Entry],
    foreign: &BTreeSet<String>,
) -> Vec<Entry> {
    if !stored.is_empty() {
        return stored.to_vec();
    }
    let mut browsers: Vec<&TargetInfo> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::App && !info.missing)
        .filter(|info| menu::app_id(info).is_none_or(|id| !foreign.contains(id)))
        .collect();
    browsers.sort_by_key(|info| info.name.to_lowercase());
    browsers
        .into_iter()
        .map(|info| Entry {
            target: info.target.clone(),
            hotkey: None,
        })
        .collect()
}

/// The patch for `entries`: arrays replace, so the whole list goes.
#[must_use]
pub fn to_patch(entries: &[Entry]) -> Value {
    let list: Vec<Value> = entries.iter().map(Entry::to_value).collect();
    json!({"browsers": {"shown": list}})
}

/// One row of the sheet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// The target as JSON text: the row's identity.
    pub key: String,
    pub target: Value,
    /// "<Profile> (<Browser>)" (SHOWN-02).
    pub name: String,
    pub icon: String,
    pub badge: Option<Value>,
    pub checked: bool,
    /// The hotkey stored for this row.
    pub hotkey: Option<String>,
    /// The hotkey to show: the stored one, or what the scheme gives when
    /// the scheme is not "Assigned per browser" (SHOWN-04).
    pub shown_hotkey: Option<String>,
    /// An app added with "+", which can be removed again (SHOWN-08).
    pub removable: bool,
    /// The app is gone (APP-10).
    pub missing: bool,
}

/// The identity of `target` in a row.
#[must_use]
pub fn key_of(target: &Value) -> String {
    target.to_string()
}

fn candidate_rank(kind: TargetKind) -> Option<u8> {
    match kind {
        TargetKind::App => Some(0),
        TargetKind::Profile => Some(1),
        TargetKind::Custom => Some(2),
        TargetKind::Private => Some(3),
        TargetKind::Picker | TargetKind::Default => None,
    }
}

fn row(inventory: &TargetInventory, info: &TargetInfo, entry: Option<&Entry>) -> Row {
    Row {
        key: key_of(&info.target),
        target: info.target.clone(),
        name: info.name.clone(),
        icon: menu::display_icon(inventory, info),
        badge: info
            .badge
            .as_ref()
            .and_then(|badge| serde_json::to_value(badge).ok()),
        checked: entry.is_some(),
        hotkey: entry.and_then(|e| e.hotkey.clone()),
        shown_hotkey: entry.and_then(|e| e.hotkey.clone()),
        removable: info.kind == TargetKind::Custom,
        missing: info.missing,
    }
}

/// The rows: checked ones in the list's order, then every other candidate
/// (SHOWN-02, SHOWN-03). Candidates are the installed browsers, every
/// profile, added apps, then private windows. A checked entry the inventory
/// does not list yet (an app just added with "Add App…", SHOWN-05) is shown
/// from `chosen`; the inventory wins, and candidates come from it alone.
#[must_use]
pub fn rows(
    inventory: &TargetInventory,
    shown: &[Entry],
    foreign: &BTreeSet<String>,
    chosen: &[TargetInfo],
) -> Vec<Row> {
    let checked = shown.iter().filter_map(|entry| {
        let info = inventory
            .targets
            .iter()
            .chain(chosen)
            .find(|info| info.target == entry.target)?;
        Some(row(inventory, info, Some(entry)))
    });
    let mut candidates: Vec<(u8, &TargetInfo)> = inventory
        .targets
        .iter()
        .filter(|info| !info.missing)
        .filter(|info| !shown.iter().any(|entry| entry.target == info.target))
        .filter(|info| menu::app_id(info).is_none_or(|id| !foreign.contains(id)))
        .filter_map(|info| candidate_rank(info.kind).map(|rank| (rank, info)))
        .collect();
    candidates.sort_by_key(|(rank, _)| *rank);
    checked
        .chain(
            candidates
                .into_iter()
                .map(|(_, info)| row(inventory, info, None)),
        )
        .collect()
}

/// Check or uncheck `target` (SHOWN-03). A new entry goes to the end.
#[must_use]
pub fn toggle(entries: &[Entry], target: &Value, checked: bool) -> Vec<Entry> {
    let present = entries.iter().any(|entry| entry.target == *target);
    match (checked, present) {
        (true, false) => entries
            .iter()
            .cloned()
            .chain([Entry {
                target: target.clone(),
                hotkey: None,
            }])
            .collect(),
        (false, true) => entries
            .iter()
            .filter(|entry| entry.target != *target)
            .cloned()
            .collect(),
        _ => entries.to_vec(),
    }
}

/// Move the entry at `from` to `to` (both indices among the checked
/// rows), when dragging reorders them (SHOWN-03). Out-of-range indices
/// change nothing.
#[must_use]
pub fn move_entry(entries: &[Entry], from: usize, to: usize) -> Vec<Entry> {
    if from >= entries.len() || to >= entries.len() || from == to {
        return entries.to_vec();
    }
    let mut next = entries.to_vec();
    let moved = next.remove(from);
    next.insert(to, moved);
    next
}

/// Give `target` the hotkey `hotkey`, or none. A hotkey is unique, so the
/// entry that had it loses it (SHOWN-04). An unchecked target is added
/// first.
#[must_use]
pub fn set_hotkey(entries: &[Entry], target: &Value, hotkey: Option<&str>) -> Vec<Entry> {
    toggle(entries, target, true)
        .into_iter()
        .map(|entry| {
            let same_key = hotkey.is_some_and(|key| entry.hotkey.as_deref() == Some(key));
            if entry.target == *target {
                Entry {
                    hotkey: hotkey.map(str::to_owned),
                    ..entry
                }
            } else if same_key {
                Entry {
                    hotkey: None,
                    ..entry
                }
            } else {
                entry
            }
        })
        .collect()
}

/// Add an app the user chose with "+" (SHOWN-05): checked, at the end.
#[must_use]
pub fn add(entries: &[Entry], target: &Value) -> Vec<Entry> {
    toggle(entries, target, true)
}

/// Remove an app added with "+" (SHOWN-08).
#[must_use]
pub fn remove(entries: &[Entry], target: &Value) -> Vec<Entry> {
    toggle(entries, target, false)
}

/// The hotkey each checked row shows under `scheme` (SHOWN-04): the labels
/// the core assigns, in the order of `rows`; unchecked rows keep none.
#[must_use]
pub fn with_scheme_hotkeys(rows: Vec<Row>, labels: &[Option<String>]) -> Vec<Row> {
    let mut labels = labels.iter();
    rows.into_iter()
        .map(|row| {
            if row.checked {
                Row {
                    shown_hotkey: labels.next().cloned().flatten(),
                    ..row
                }
            } else {
                row
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
