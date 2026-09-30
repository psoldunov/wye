//! `GetApps`, `GetServices` and `GetExpansionCatalogue` payloads (DLG-APP,
//! APP-03, APP-05, DLG-EXP). Pure.

use wye_api::apps::{AppInfo, AppList};
use wye_api::expansion::{ExpansionCatalogue, ShortLink, Wrapper};
use wye_api::services::{ServiceInfo, ServiceList};
use wye_core::config::ExpansionSettings;
use wye_core::expand;
use wye_core::{Config, ServiceCatalogue, Target};
use wye_desktop::{Inventory, Locale};

use super::targets::{packaging, spec};

/// DLG-APP-01: installed apps (browsers only unless `all`), by name, and the
/// apps that recently sent links (DLG-APP-02).
pub(crate) fn apps(
    inventory: &Inventory,
    locale: &Locale,
    all: bool,
    recent_sources: Vec<String>,
) -> AppList {
    let mut apps: Vec<AppInfo> = inventory
        .apps()
        .filter(|app| !app.forwards_links)
        .filter(|app| all || app.handles_web)
        .map(|app| AppInfo {
            id: app.id().to_string(),
            name: app.display_name(locale),
            generic_name: app.entry.generic_name_in(locale),
            keywords: app.entry.keywords_in(locale),
            icon: app.entry.icon.clone(),
            packaging: Some(packaging(app.packaging)),
            is_browser: app.handles_web,
        })
        .collect();
    apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.id.clone()));
    AppList {
        apps,
        recent_sources,
    }
}

/// APP-03, APP-05: the catalogue with each service's installed own app and
/// its mapping (`{"default": true}` when none is stored).
pub(crate) fn services(
    inventory: &Inventory,
    locale: &Locale,
    config: &Config,
    catalogue: &ServiceCatalogue,
) -> ServiceList {
    let services = catalogue
        .services()
        .iter()
        .map(|service| {
            let installed = service
                .desktop_apps
                .iter()
                .find_map(|id| inventory.get(id).filter(|app| !app.forwards_links));
            ServiceInfo {
                id: service.id.clone(),
                name: service.name.clone(),
                icon: installed.and_then(|app| app.entry.icon.clone()),
                installed_app: installed.map(|app| wye_api::AppRef {
                    id: app.id().to_string(),
                    name: app.display_name(locale),
                    icon: app.entry.icon.clone(),
                }),
                target: spec(config.apps.get(&service.id).unwrap_or(&Target::Default)),
            }
        })
        .collect();
    ServiceList { services }
}

/// DLG-EXP-01, DLG-EXP-02: wrappers and short-link domains with their
/// switches; the user's own short-link domains come last.
pub(crate) fn expansion(
    catalogue: &expand::ExpansionCatalogue,
    settings: &ExpansionSettings,
) -> ExpansionCatalogue {
    let wrappers = catalogue
        .wrappers()
        .iter()
        .map(|wrapper| Wrapper {
            id: wrapper.id.clone(),
            name: wrapper.name.clone(),
            hosts: wrapper.hosts().to_vec(),
            enabled: settings.is_enabled(&wrapper.id),
        })
        .collect();
    let short_links = catalogue
        .short_links()
        .iter()
        .chain(settings.custom_short_links.iter())
        .map(|domain| ShortLink {
            domain: domain.clone(),
            enabled: settings.is_enabled(domain),
        })
        .collect();
    ExpansionCatalogue {
        wrappers,
        short_links,
    }
}
