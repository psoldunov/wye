//! The target menu (TGT-01 to TGT-07): the popup every target popup row
//! opens, built from `GetTargets` as flat rows QML draws in a list.
//!
//! Sections, in order (TGT-02), separated by a separator row:
//!
//! 1. "Default (<primary>)": Apps page and rule editor only.
//! 2. "Picker": every menu (TGT-07).
//! 3. The service's own desktop app: Apps page, when installed (APP-05).
//! 4. Every installed browser and every added app, alphabetically (TGT-05).
//! 5. "Private Browsing" and one "<Browser> (Private)" per browser.
//! 6. "Profiles: <Browser>" and its profiles, per browser.
//! 7. "Other…", which opens the app chooser (TGT-06).
//!
//! The current value has the checkmark (TGT-03). A current value whose app
//! is gone leads the menu as a missing item (APP-10).

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::services::ServiceInfo;
use wye_api::targets::{TargetInfo, TargetInventory, TargetKind};
use wye_core::target_menu::{OTHER_LABEL, PICKER_LABEL, PRIVATE_HEADER};

use super::icon;

/// Where the menu is shown, which decides its sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Primary and alternative browser (BRW-01, BRW-02).
    Browsers,
    /// A web app mapping (APP-04).
    Apps,
    /// The rule editor's "Open in" (RUL-12).
    Rule,
}

impl Surface {
    /// The name QML passes.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "browsers" => Some(Self::Browsers),
            "apps" => Some(Self::Apps),
            "rule" => Some(Self::Rule),
            _ => None,
        }
    }

    const fn has_default(self) -> bool {
        matches!(self, Self::Apps | Self::Rule)
    }
}

/// What a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RowKind {
    /// A target to choose.
    Item,
    /// "Other…": open the app chooser.
    Other,
    /// A dimmed section header.
    Header,
    /// A separator line.
    Separator,
}

/// One row of the menu.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub kind: RowKind,
    pub label: String,
    /// `Kirigami.Icon.source`, empty for none.
    pub icon: String,
    /// The profile badge (`{"initial", "color"}` or `{"image"}`).
    pub badge: Option<Value>,
    /// The target in the configuration's shape; `null` for non-items.
    pub target: Value,
    pub checked: bool,
    /// The app is gone (APP-10): drawn with a warning icon.
    pub missing: bool,
}

impl Row {
    fn plain(kind: RowKind, label: &str) -> Self {
        Self {
            kind,
            label: label.to_owned(),
            icon: String::new(),
            badge: None,
            target: Value::Null,
            checked: false,
            missing: false,
        }
    }

    fn item(label: &str, icon: String, target: Value, current: &Value) -> Self {
        Self {
            checked: target == *current,
            label: label.to_owned(),
            icon,
            target,
            ..Self::plain(RowKind::Item, label)
        }
    }

    fn of(info: &TargetInfo, label: &str, icon: String, current: &Value) -> Self {
        Self {
            badge: info
                .badge
                .as_ref()
                .and_then(|b| serde_json::to_value(b).ok()),
            missing: info.missing,
            ..Self::item(label, icon, info.target.clone(), current)
        }
    }
}

/// What to build the menu from.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub surface: Surface,
    /// The current value; gets the checkmark (TGT-03).
    pub current: &'a Value,
    /// The service being mapped, on the Apps page (TGT-02 c).
    pub service: Option<&'a ServiceInfo>,
    /// Every service, to keep other services' own apps out of the browser
    /// section (TGT-05).
    pub services: &'a [ServiceInfo],
    /// The primary browser's name, for the Default item (TGT-02 a).
    pub primary_name: &'a str,
}

/// The menu's rows, top to bottom.
#[must_use]
pub fn build(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Row> {
    let sections = [
        default_section(request),
        picker_section(request),
        own_app_section(inventory, request),
        missing_section(inventory, request),
        apps_section(inventory, request),
        private_section(inventory, request),
    ]
    .into_iter()
    .chain(profile_sections(inventory, request))
    .chain([other_section()])
    .filter(|section| !section.is_empty());
    let mut rows = Vec::new();
    for section in sections {
        if !rows.is_empty() {
            rows.push(Row::plain(RowKind::Separator, ""));
        }
        rows.extend(section);
    }
    rows
}

/// The row for `current` alone: what the closed popup row shows (TGT-01).
/// A target the inventory does not know shows as missing (APP-10).
#[must_use]
pub fn describe(inventory: &TargetInventory, request: &Request<'_>) -> Row {
    if *request.current == json!({"default": true}) {
        return default_row(request);
    }
    if let Some(info) = inventory
        .targets
        .iter()
        .find(|info| info.target == *request.current)
    {
        let icon = display_icon(inventory, info);
        return Row::of(info, &info.name, icon, request.current);
    }
    if let Some(app) = request.service.and_then(|s| s.installed_app.as_ref())
        && json!({"app": app.id}) == *request.current
    {
        let icon = icon::source(app.icon.as_deref());
        return Row::item(&app.name, icon, request.current.clone(), request.current);
    }
    Row {
        missing: true,
        ..Row::item(
            &fallback_name(request.current),
            String::new(),
            request.current.clone(),
            request.current,
        )
    }
}

fn fallback_name(target: &Value) -> String {
    target
        .as_object()
        .and_then(|table| table.values().next())
        .map_or_else(
            || "Unknown".to_owned(),
            |value| match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            },
        )
}

fn default_row(request: &Request<'_>) -> Row {
    let label = format!("Default ({})", request.primary_name);
    Row::item(
        &label,
        "go-home".to_owned(),
        json!({"default": true}),
        request.current,
    )
}

fn default_section(request: &Request<'_>) -> Vec<Row> {
    if request.surface.has_default() {
        vec![default_row(request)]
    } else {
        Vec::new()
    }
}

fn picker_section(request: &Request<'_>) -> Vec<Row> {
    vec![Row::item(
        PICKER_LABEL,
        icon::PICKER_ICON.to_owned(),
        json!({"picker": true}),
        request.current,
    )]
}

fn own_app_section(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Row> {
    if request.surface != Surface::Apps {
        return Vec::new();
    }
    let Some(app) = request.service.and_then(|s| s.installed_app.as_ref()) else {
        return Vec::new();
    };
    let target = json!({"app": app.id});
    let known = inventory.targets.iter().find(|info| info.target == target);
    let icon = icon::source(
        app.icon
            .as_deref()
            .or_else(|| known.and_then(|k| k.icon.as_deref())),
    );
    vec![Row::item(&app.name, icon, target, request.current)]
}

/// The desktop IDs of the apps web services own. They are not browsers, so
/// no browser menu or list offers them (TGT-05, SHOWN-02).
pub fn foreign_app_ids(services: &[ServiceInfo]) -> BTreeSet<String> {
    services
        .iter()
        .filter_map(|service| service.installed_app.as_ref())
        .map(|app| app.id.clone())
        .collect()
}

pub(super) fn is_app_target(info: &TargetInfo) -> bool {
    matches!(info.kind, TargetKind::App | TargetKind::Custom)
}

pub(super) fn app_id(info: &TargetInfo) -> Option<&str> {
    info.target.get("app").and_then(Value::as_str)
}

fn apps_section(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Row> {
    let foreign = foreign_app_ids(request.services);
    let mut infos: Vec<&TargetInfo> = inventory
        .targets
        .iter()
        .filter(|info| is_app_target(info) && !info.missing)
        .filter(|info| app_id(info).is_none_or(|id| !foreign.contains(id)))
        .collect();
    infos.sort_by_key(|info| info.name.to_lowercase());
    infos
        .into_iter()
        .map(|info| {
            Row::of(
                info,
                &info.name,
                icon::source(info.icon.as_deref()),
                request.current,
            )
        })
        .collect()
}

/// The browser a private or profile target belongs to.
fn browser_of<'a>(inventory: &'a TargetInventory, info: &TargetInfo) -> Option<&'a TargetInfo> {
    let id = info.browser.as_deref()?;
    let target = json!({"app": id});
    inventory
        .targets
        .iter()
        .find(|candidate| candidate.target == target)
}

pub(super) fn display_icon(inventory: &TargetInventory, info: &TargetInfo) -> String {
    let own = icon::source(info.icon.as_deref());
    if !own.is_empty() || !matches!(info.kind, TargetKind::Private | TargetKind::Profile) {
        return own;
    }
    // TGT-03: a private or profile item shows its browser's icon.
    browser_of(inventory, info)
        .map_or_else(String::new, |browser| icon::source(browser.icon.as_deref()))
}

fn private_section(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Row> {
    let mut infos: Vec<&TargetInfo> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::Private && !info.missing)
        .collect();
    infos.sort_by_key(|info| info.name.to_lowercase());
    if infos.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![Row::plain(RowKind::Header, PRIVATE_HEADER)];
    rows.extend(infos.into_iter().map(|info| {
        Row::of(
            info,
            &info.name,
            display_icon(inventory, info),
            request.current,
        )
    }));
    rows
}

fn profile_sections(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Vec<Row>> {
    let browsers: BTreeSet<&str> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::Profile && !info.missing)
        .filter_map(|info| info.browser.as_deref())
        .collect();
    let mut sections: Vec<(String, Vec<Row>)> = browsers
        .into_iter()
        .map(|browser| {
            let name = inventory
                .targets
                .iter()
                .find(|info| info.target == json!({"app": browser}))
                .map_or_else(|| browser.to_owned(), |info| info.name.clone());
            let mut rows = vec![Row::plain(RowKind::Header, &format!("Profiles: {name}"))];
            rows.extend(
                inventory
                    .targets
                    .iter()
                    .filter(|info| info.kind == TargetKind::Profile && !info.missing)
                    .filter(|info| info.browser.as_deref() == Some(browser))
                    .map(|info| {
                        let label = info.short_name.as_deref().unwrap_or(&info.name);
                        Row::of(info, label, display_icon(inventory, info), request.current)
                    }),
            );
            (name.to_lowercase(), rows)
        })
        .collect();
    sections.sort_by(|a, b| a.0.cmp(&b.0));
    sections.into_iter().map(|(_, rows)| rows).collect()
}

/// APP-10: the current value's app is gone; keep it visible and marked.
fn missing_section(inventory: &TargetInventory, request: &Request<'_>) -> Vec<Row> {
    inventory
        .targets
        .iter()
        .filter(|info| info.missing && info.target == *request.current)
        .map(|info| Row::of(info, &info.name, String::new(), request.current))
        .collect()
}

fn other_section() -> Vec<Row> {
    vec![Row::plain(RowKind::Other, OTHER_LABEL)]
}

#[cfg(test)]
mod tests;
