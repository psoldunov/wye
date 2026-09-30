//! `GetTargets`: every target with its name, icon, badge and abilities
//! (TGT-02 to TGT-07, SHOWN-02, PICK-06, DISC-08, APP-05, APP-10). Pure.

use std::collections::BTreeSet;

use wye_api::targets::{TargetInfo, TargetInventory, TargetKind};
use wye_api::{Badge, Packaging, TargetCapabilities};
use wye_core::target_menu::{self, CustomEntry, PICKER_LABEL, TargetCatalog};
use wye_core::{Availability as _, Config, CustomApp, DesktopId, ServiceCatalogue, Target};
use wye_desktop::{Inventory, Locale};

/// The catalogue menus and the picker read: web handlers, the installed own
/// apps of web services (APP-05) and the custom apps the configuration
/// names (TGT-06).
pub(crate) fn catalog(
    inventory: &Inventory,
    locale: &Locale,
    config: &Config,
    services: &ServiceCatalogue,
) -> TargetCatalog {
    let own_apps: Vec<DesktopId> = services
        .services()
        .iter()
        .flat_map(|service| service.desktop_apps.iter().cloned())
        .filter(|id| inventory.get(id).is_some())
        .collect();
    TargetCatalog {
        custom: custom_entries(inventory, locale, config),
        ..inventory.catalog(locale, &own_apps)
    }
}

/// The inventory `GetTargets` returns: the Picker, every available target in
/// TGT-02 order, the service apps, then the configured targets whose app is
/// gone, marked missing (APP-10).
pub(crate) fn inventory(
    inventory: &Inventory,
    locale: &Locale,
    config: &Config,
    services: &ServiceCatalogue,
) -> TargetInventory {
    let catalog = catalog(inventory, locale, config, services);
    let picker = TargetInfo {
        target: spec(&Target::Picker),
        kind: TargetKind::Picker,
        name: PICKER_LABEL.to_owned(),
        short_name: None,
        icon: None,
        badge: None,
        browser: None,
        capabilities: TargetCapabilities::default(),
        packaging: None,
        missing: false,
    };
    let own_apps = catalog
        .apps
        .iter()
        .filter_map(|app| catalog.describe(&Target::App(app.app.clone())));
    let available = catalog.all().into_iter().chain(own_apps);
    let mut seen = BTreeSet::new();
    let described: Vec<TargetInfo> = available
        .filter(|info| seen.insert(info.target.to_string()))
        .map(|info| describe(&info, inventory))
        .collect();
    let missing = configured(config)
        .into_iter()
        .filter(|target| target.is_concrete() && !catalog.is_available(target))
        .filter(|target| seen.insert(target.to_string()))
        .map(|target| missing_info(&target));
    TargetInventory {
        targets: std::iter::once(picker)
            .chain(described)
            .chain(missing)
            .collect(),
    }
}

fn describe(info: &target_menu::TargetInfo, inventory: &Inventory) -> TargetInfo {
    let browser = match &info.target {
        Target::Private(id) | Target::Profile { app: id, .. } => Some(id.to_string()),
        _ => None,
    };
    let app = match &info.target {
        Target::App(id)
        | Target::Private(id)
        | Target::Profile { app: id, .. }
        | Target::Custom(CustomApp::Desktop(id)) => inventory.get(id),
        _ => None,
    };
    TargetInfo {
        target: spec(&info.target),
        kind: kind(&info.target),
        name: info.long_name.clone(),
        short_name: (info.name != info.long_name).then(|| info.name.clone()),
        icon: info.icon.clone(),
        badge: info.badge.as_ref().map(badge),
        browser,
        capabilities: TargetCapabilities {
            private: info.caps.private,
            new_window: info.caps.new_window,
            // Best effort for every target (LAUNCH-04).
            background: true,
        },
        packaging: app.map(|app| packaging(app.packaging)),
        missing: false,
    }
}

fn missing_info(target: &Target) -> TargetInfo {
    TargetInfo {
        target: spec(target),
        kind: kind(target),
        name: target.to_string(),
        short_name: None,
        icon: None,
        badge: None,
        browser: None,
        capabilities: TargetCapabilities::default(),
        packaging: None,
        missing: true,
    }
}

/// Every concrete target the configuration names.
fn configured(config: &Config) -> Vec<Target> {
    let browsers = [&config.browsers.primary, &config.browsers.alternative];
    browsers
        .into_iter()
        .chain(config.browsers.shown.iter().map(|entry| &entry.target))
        .chain(config.apps.values())
        .chain(config.rules.iter().map(|rule| &rule.target))
        .cloned()
        .collect()
}

/// TGT-06: custom apps the configuration names, with the entry's name or
/// the executable's file name.
fn custom_entries(inventory: &Inventory, locale: &Locale, config: &Config) -> Vec<CustomEntry> {
    let mut seen = BTreeSet::new();
    configured(config)
        .into_iter()
        .filter_map(|target| match target {
            Target::Custom(app) => Some(app),
            _ => None,
        })
        .filter(|app| seen.insert(app.to_string()))
        .filter_map(|app| match &app {
            CustomApp::Desktop(id) => inventory.get(id).map(|found| CustomEntry {
                name: found.display_name(locale),
                icon: found.entry.icon.clone(),
                app: app.clone(),
            }),
            CustomApp::Executable(path) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
                Some(CustomEntry {
                    name,
                    icon: None,
                    app: app.clone(),
                })
            }
        })
        .collect()
}

/// A target in the configuration's JSON shape.
pub(crate) fn spec(target: &Target) -> serde_json::Value {
    serde_json::to_value(target).unwrap_or(serde_json::Value::Null)
}

pub(crate) fn kind(target: &Target) -> TargetKind {
    match target {
        Target::Picker => TargetKind::Picker,
        Target::Default => TargetKind::Default,
        Target::App(_) => TargetKind::App,
        Target::Private(_) => TargetKind::Private,
        Target::Profile { .. } => TargetKind::Profile,
        Target::Custom(_) => TargetKind::Custom,
    }
}

/// DISC-08: the core's badge as the wire shape.
pub(crate) fn badge(badge: &target_menu::Badge) -> Badge {
    match badge {
        target_menu::Badge::Image(path) => Badge::Image {
            image: path.clone(),
        },
        target_menu::Badge::Initial { text, color } => Badge::Initial {
            initial: text.clone(),
            color: format!("#{:06x}", color & 0x00ff_ffff),
        },
    }
}

pub(crate) fn packaging(packaging: wye_desktop::Packaging) -> Packaging {
    match packaging {
        wye_desktop::Packaging::Native => Packaging::Native,
        wye_desktop::Packaging::Flatpak => Packaging::Flatpak,
        wye_desktop::Packaging::Snap => Packaging::Snap,
    }
}
