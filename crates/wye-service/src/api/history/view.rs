//! History entries as `GetHistory` returns them (DLG-HIS-02). Pure.

use wye_api::context::Entry;
use wye_api::history::HistoryEntry as WireEntry;
use wye_core::history::HistoryEntry;
use wye_core::pipeline::EntryPoint;
use wye_core::target_menu::TargetCatalog;
use wye_core::{CustomApp, DesktopId, Target};
use wye_desktop::{InstalledApp, Inventory, Locale};

use crate::api::inventory::targets::spec;

/// One stored entry with display names for its source and target.
pub(crate) fn entry(
    stored: &HistoryEntry,
    catalog: &TargetCatalog,
    inventory: &Inventory,
    locale: &Locale,
) -> WireEntry {
    let (target_name, target_icon) = target_display(&stored.target, catalog, inventory, locale);
    WireEntry {
        id: stored.id,
        time: stored.time,
        original_url: stored.original.clone(),
        final_url: stored.url.clone(),
        entry: entry_point(stored.entry),
        source: stored.source.clone(),
        source_name: stored
            .source
            .as_deref()
            .and_then(|source| DesktopId::new(source).ok())
            .and_then(|id| inventory.get(&id))
            .map(|app| app.display_name(locale)),
        target: spec(&stored.target),
        target_name,
        target_icon,
        reason: stored.reason.label(),
        cleaned: stored.cleaned,
        expanded: stored.expanded,
    }
}

/// The target's long name ("Work (Chrome)"), or the target itself when its
/// app is gone.
pub(crate) fn target_name(target: &Target, catalog: &TargetCatalog) -> String {
    catalog
        .describe(target)
        .map_or_else(|| target.to_string(), |info| info.long_name)
}

/// The target's long name and icon: the catalogue's, else the desktop
/// entry's for an installed app the catalogue does not hold, else the
/// target itself with no icon (DLG-HIS-02).
fn target_display(
    target: &Target,
    catalog: &TargetCatalog,
    inventory: &Inventory,
    locale: &Locale,
) -> (String, Option<String>) {
    if let Some(info) = catalog.describe(target) {
        return (info.long_name, info.icon);
    }
    match installed(target, inventory) {
        Some(app) => (app.display_name(locale), app.entry.icon.clone()),
        None => (target.to_string(), None),
    }
}

/// The installed app of an app or custom-app target. The catalogue holds
/// only web handlers, service apps and the custom apps the configuration
/// names, so a history entry may name an app it does not hold.
fn installed<'a>(target: &Target, inventory: &'a Inventory) -> Option<&'a InstalledApp> {
    match target {
        Target::App(id) | Target::Custom(CustomApp::Desktop(id)) => inventory.get(id),
        _ => None,
    }
}

pub(crate) const fn entry_point(entry: EntryPoint) -> Entry {
    match entry {
        EntryPoint::Handler => Entry::Handler,
        EntryPoint::Clipboard => Entry::Clipboard,
        EntryPoint::Extension => Entry::Extension,
        EntryPoint::Cli => Entry::Cli,
    }
}
