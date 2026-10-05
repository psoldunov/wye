//! The browsers step's data (ONB-03): the primary browser menu, the
//! checklist of browsers and profiles for the picker, the first six
//! browsers pre-checked, and the merge patches each choice becomes.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::AppRef;
use wye_api::services::ServiceInfo;
use wye_api::targets::{TargetInfo, TargetInventory, TargetKind};
use wye_core::Target;
use wye_core::merge_patch::target_patch;

use crate::settings::icon;

/// How many browsers are checked at first (ONB-03).
pub const PRESELECTED: usize = 6;

/// The Picker as a target in the configuration's shape (TGT-02).
fn picker_target() -> Value {
    json!({"picker": true})
}

/// The desktop ID of an app target.
fn app_id(target: &Value) -> Option<&str> {
    target.get("app").and_then(Value::as_str)
}

/// The desktop IDs of the apps web services own (APP-05, APP-12): not
/// browsers, so no list here offers them (TGT-05, SHOWN-02).
#[must_use]
pub fn foreign_app_ids(services: &[ServiceInfo]) -> BTreeSet<String> {
    services
        .iter()
        .flat_map(ServiceInfo::own_apps)
        .map(|app| app.id.clone())
        .collect()
}

/// The installed browsers, alphabetically (the order the picker uses when
/// the user chose none).
fn browsers<'a>(inventory: &'a TargetInventory, foreign: &BTreeSet<String>) -> Vec<&'a TargetInfo> {
    let mut found: Vec<&TargetInfo> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::App && !info.missing)
        .filter(|info| app_id(&info.target).is_none_or(|id| !foreign.contains(id)))
        .collect();
    found.sort_by_key(|info| info.name.to_lowercase());
    found
}

/// Every profile of an installed browser, alphabetically.
fn profiles<'a>(inventory: &'a TargetInventory, foreign: &BTreeSet<String>) -> Vec<&'a TargetInfo> {
    let mut found: Vec<&TargetInfo> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::Profile && !info.missing)
        .filter(|info| {
            info.browser
                .as_deref()
                .is_none_or(|browser| !foreign.contains(browser))
        })
        .collect();
    found.sort_by_key(|info| info.name.to_lowercase());
    found
}

/// The entries of `browsers.shown` as the configuration holds them.
#[must_use]
pub fn stored_shown(config: &Value) -> Vec<Value> {
    config
        .pointer("/browsers/shown")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn entry_target(entry: &Value) -> Option<&Value> {
    entry.get("target")
}

/// The first [`PRESELECTED`] browsers: no profiles, no private windows
/// (ONB-03).
#[must_use]
pub fn initial_shown(inventory: &TargetInventory, foreign: &BTreeSet<String>) -> Vec<Value> {
    browsers(inventory, foreign)
        .into_iter()
        .take(PRESELECTED)
        .map(|info| json!({"target": info.target}))
        .collect()
}

/// What the checklist shows as checked: the user's list, or, while the
/// configuration has none, the pre-checked browsers (ONB-03).
#[must_use]
pub fn effective_shown(
    config: &Value,
    inventory: &TargetInventory,
    foreign: &BTreeSet<String>,
) -> Vec<Value> {
    let stored = stored_shown(config);
    if stored.is_empty() {
        initial_shown(inventory, foreign)
    } else {
        stored
    }
}

/// The patch that stores the pre-checked browsers, or `None` when the
/// configuration already has a list or there is nothing to store (ONB-03).
#[must_use]
pub fn seed_patch(
    config: &Value,
    inventory: &TargetInventory,
    foreign: &BTreeSet<String>,
) -> Option<Value> {
    if !stored_shown(config).is_empty() {
        return None;
    }
    let initial = initial_shown(inventory, foreign);
    (!initial.is_empty()).then(|| shown_patch(&initial))
}

/// The patch for a new shown list: arrays replace, so the whole list goes.
#[must_use]
pub fn shown_patch(shown: &[Value]) -> Value {
    json!({"browsers": {"shown": shown}})
}

/// The patch that makes `target` the primary browser (BRW-01, ONB-03).
///
/// The configuration holds a target as a table with one key, so the patch
/// clears every other kind ([`target_patch`]): a plain merge patch would put
/// `picker` next to the stored `app`, which the service refuses.
///
/// # Errors
///
/// The reason `target` is not a target.
pub fn primary_patch(target: &Value) -> Result<Value, serde_json::Error> {
    let target: Target = serde_json::from_value(target.clone())?;
    Ok(json!({"browsers": {"primary": target_patch(&target)}}))
}

/// The patch for the **Launch at login** switch (GEN-01).
#[must_use]
pub fn launch_patch(on: bool) -> Value {
    json!({"general": {"launch-at-login": on}})
}

/// `shown` with `target` checked or unchecked; a new entry goes last and a
/// kept entry keeps its hotkey.
#[must_use]
pub fn toggled(shown: &[Value], target: &Value, checked: bool) -> Vec<Value> {
    let present = shown
        .iter()
        .any(|entry| entry_target(entry) == Some(target));
    match (checked, present) {
        (true, false) => shown
            .iter()
            .cloned()
            .chain([json!({"target": target})])
            .collect(),
        (false, true) => shown
            .iter()
            .filter(|entry| entry_target(entry) != Some(target))
            .cloned()
            .collect(),
        _ => shown.to_vec(),
    }
}

/// One row of the checklist (`WyeChecklist` takes `key`, `name`, `icon`,
/// `checked`; the rest is ours).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRow {
    /// The target as JSON text: the row's identity.
    pub key: String,
    pub target: Value,
    pub name: String,
    pub icon: String,
    pub checked: bool,
    pub missing: bool,
    pub removable: bool,
}

/// The checklist: checked rows first, in the list's order, then the other
/// browsers, then the other profiles (ONB-03).
#[must_use]
pub fn checklist(
    inventory: &TargetInventory,
    foreign: &BTreeSet<String>,
    shown: &[Value],
) -> Vec<ListRow> {
    let row = |info: &TargetInfo, checked: bool| ListRow {
        key: info.target.to_string(),
        target: info.target.clone(),
        name: info.name.clone(),
        icon: icon::source(info.icon.as_deref()),
        checked,
        missing: false,
        removable: false,
    };
    let listed = |target: &Value| {
        shown
            .iter()
            .any(|entry| entry_target(entry) == Some(target))
    };
    let checked = shown.iter().filter_map(|entry| {
        let target = entry_target(entry)?;
        let info = inventory
            .targets
            .iter()
            .find(|info| &info.target == target)?;
        (matches!(info.kind, TargetKind::App | TargetKind::Profile) && !info.missing)
            .then(|| row(info, true))
    });
    let unchecked = browsers(inventory, foreign)
        .into_iter()
        .chain(profiles(inventory, foreign))
        .filter(|info| !listed(&info.target))
        .map(|info| row(info, false));
    checked.chain(unchecked).collect()
}

/// One entry of the primary browser popup (ONB-03).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryChoice {
    pub target: Value,
    pub name: String,
    pub icon: String,
    pub checked: bool,
}

/// The primary browser menu: the browser Wye replaced first, then the
/// Picker, then the other browsers (ONB-03). `current` is
/// `browsers.primary`; a missing one is the Picker.
#[must_use]
pub fn primary_choices(
    inventory: &TargetInventory,
    foreign: &BTreeSet<String>,
    previous_default: Option<&AppRef>,
    current: Option<&Value>,
) -> Vec<PrimaryChoice> {
    let current = current.cloned().unwrap_or_else(picker_target);
    let choice = |target: &Value, name: &str, icon: String| PrimaryChoice {
        checked: *target == current,
        target: target.clone(),
        name: name.to_owned(),
        icon,
    };
    let browser_choice =
        |info: &TargetInfo| choice(&info.target, &info.name, icon::source(info.icon.as_deref()));
    let all = browsers(inventory, foreign);
    let previous = previous_default.and_then(|app| {
        all.iter()
            .find(|info| app_id(&info.target) == Some(app.id.as_str()))
            .copied()
    });
    let picker = inventory
        .targets
        .iter()
        .find(|info| info.kind == TargetKind::Picker);
    let picker_choice = choice(
        &picker_target(),
        picker.map_or("Picker", |info| info.name.as_str()),
        picker.and_then(|info| info.icon.as_deref()).map_or_else(
            || icon::PICKER_ICON.to_owned(),
            |name| icon::source(Some(name)),
        ),
    );
    let listed: Vec<PrimaryChoice> = previous
        .map(browser_choice)
        .into_iter()
        .chain([picker_choice])
        .chain(
            all.into_iter()
                .filter(|info| previous.is_none_or(|kept| kept.target != info.target))
                .map(browser_choice),
        )
        .collect();
    // A primary the list does not hold (a profile, a private window, a
    // custom app) goes last, so the popup shows what is set instead of its
    // first entry.
    let configured = listed
        .iter()
        .all(|choice| !choice.checked)
        .then(|| {
            inventory
                .targets
                .iter()
                .find(|info| info.target == current && !info.missing)
        })
        .flatten()
        .map(browser_choice);
    listed.into_iter().chain(configured).collect()
}

#[cfg(test)]
mod tests;
