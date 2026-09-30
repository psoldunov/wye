//! History entries as `GetHistory` returns them (DLG-HIS-02). Pure.

use wye_api::context::Entry;
use wye_api::history::HistoryEntry as WireEntry;
use wye_core::history::HistoryEntry;
use wye_core::pipeline::EntryPoint;
use wye_core::target_menu::TargetCatalog;
use wye_core::{DesktopId, Target};
use wye_desktop::{Inventory, Locale};

use crate::api::inventory::targets::spec;

/// One stored entry with display names for its source and target.
pub(crate) fn entry(
    stored: &HistoryEntry,
    catalog: &TargetCatalog,
    inventory: &Inventory,
    locale: &Locale,
) -> WireEntry {
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
        target_name: target_name(&stored.target, catalog),
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

pub(crate) const fn entry_point(entry: EntryPoint) -> Entry {
    match entry {
        EntryPoint::Handler => Entry::Handler,
        EntryPoint::Clipboard => Entry::Clipboard,
        EntryPoint::Extension => Entry::Extension,
        EntryPoint::Cli => Entry::Cli,
    }
}
