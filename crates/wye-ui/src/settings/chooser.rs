//! The app chooser (DLG-APP-01 to DLG-APP-04): which rows the sheet lists
//! for a search, and the target a chosen app becomes.

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::apps::{AppInfo, AppList};
use wye_api::targets::{TargetInfo, TargetKind};
use wye_api::{Packaging, TargetCapabilities};

use super::icon;

/// "Recent Sources" (DLG-APP-02).
pub const RECENT_HEADER: &str = "Recent Sources";
/// "Browsers" (DLG-APP-02).
pub const BROWSERS_HEADER: &str = "Browsers";
/// "All Apps" (DLG-APP-02).
pub const ALL_HEADER: &str = "All Apps";

/// What a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RowKind {
    Header,
    App,
}

/// One row of the sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub kind: RowKind,
    /// The header text, or the app's name.
    pub label: String,
    /// The desktop ID; empty for a header.
    pub id: String,
    pub icon: String,
    /// "Flatpak" or "Snap", dimmed after the name (DLG-APP-02).
    pub packaging: String,
    pub is_browser: bool,
}

impl Row {
    fn header(label: &str) -> Self {
        Self {
            kind: RowKind::Header,
            label: label.to_owned(),
            id: String::new(),
            icon: String::new(),
            packaging: String::new(),
            is_browser: false,
        }
    }

    fn app(app: &AppInfo) -> Self {
        Self {
            kind: RowKind::App,
            label: app.name.clone(),
            id: app.id.clone(),
            icon: icon::source(app.icon.as_deref()),
            packaging: match app.packaging {
                Some(Packaging::Flatpak) => "Flatpak".to_owned(),
                Some(Packaging::Snap) => "Snap".to_owned(),
                Some(Packaging::Native) | None => String::new(),
            },
            is_browser: app.is_browser,
        }
    }
}

/// Whether `app` matches the search: its name, generic name, desktop ID or
/// a keyword contains every word of `search`, ignoring case (DLG-APP-01).
#[must_use]
pub fn matches(app: &AppInfo, search: &str) -> bool {
    let haystack: Vec<String> = [
        Some(app.name.as_str()),
        app.generic_name.as_deref(),
        Some(app.id.as_str()),
    ]
    .into_iter()
    .flatten()
    .chain(app.keywords.iter().map(String::as_str))
    .map(str::to_lowercase)
    .collect();
    search
        .split_whitespace()
        .map(str::to_lowercase)
        .all(|word| haystack.iter().any(|text| text.contains(&word)))
}

fn by_name(apps: &mut [&AppInfo]) {
    apps.sort_by_key(|app| app.name.to_lowercase());
}

/// The rows for `search` (DLG-APP-01, DLG-APP-02): Recent Sources (only
/// when `recent` is set, choosing source apps), Browsers, All Apps. An app
/// is listed once, in the first section that has it. Empty sections have no
/// header.
#[must_use]
pub fn rows(list: &AppList, search: &str, recent: bool) -> Vec<Row> {
    let found: Vec<&AppInfo> = list
        .apps
        .iter()
        .filter(|app| matches(app, search))
        .collect();
    let recent_apps: Vec<&AppInfo> = if recent {
        list.recent_sources
            .iter()
            .filter_map(|id| found.iter().copied().find(|app| app.id == *id))
            .collect()
    } else {
        Vec::new()
    };
    let is_recent = |app: &&AppInfo| recent_apps.iter().any(|r| r.id == app.id);
    let mut browsers: Vec<&AppInfo> = found
        .iter()
        .copied()
        .filter(|a| a.is_browser)
        .filter(|a| !is_recent(a))
        .collect();
    let mut others: Vec<&AppInfo> = found
        .iter()
        .copied()
        .filter(|a| !a.is_browser)
        .filter(|a| !is_recent(a))
        .collect();
    by_name(&mut browsers);
    by_name(&mut others);
    [
        (RECENT_HEADER, recent_apps),
        (BROWSERS_HEADER, browsers),
        (ALL_HEADER, others),
    ]
    .into_iter()
    .filter(|(_, apps)| !apps.is_empty())
    .flat_map(|(header, apps)| {
        std::iter::once(Row::header(header)).chain(apps.into_iter().map(Row::app))
    })
    .collect()
}

/// The target a chosen app becomes (TGT-06): a browser is an app target,
/// any other app a custom one.
#[must_use]
pub fn target_for(app: &AppInfo) -> Value {
    if app.is_browser {
        json!({"app": app.id})
    } else {
        json!({"custom": app.id})
    }
}

/// The target for a file picked with "Browse…" (DLG-APP-04): a `.desktop`
/// file is its desktop ID (targets never hold paths into the store or a
/// package), anything else the executable's path.
#[must_use]
pub fn browse_target(path: &str) -> Option<Value> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let name = std::path::Path::new(path).file_name()?.to_string_lossy();
    if name.ends_with(".desktop") {
        Some(json!({"custom": name}))
    } else {
        Some(json!({"custom": path}))
    }
}

fn chosen(target: Value, kind: TargetKind, name: String, icon: Option<String>) -> TargetInfo {
    TargetInfo {
        target,
        kind,
        name,
        short_name: None,
        icon,
        badge: None,
        browser: None,
        capabilities: TargetCapabilities::default(),
        packaging: None,
        missing: false,
    }
}

/// How the service would list the app just chosen (TGT-01, TGT-06), so its
/// row has a name and an icon before the service lists it.
#[must_use]
pub fn chosen_info(app: &AppInfo) -> TargetInfo {
    let kind = if app.is_browser {
        TargetKind::App
    } else {
        TargetKind::Custom
    };
    TargetInfo {
        packaging: app.packaging,
        ..chosen(target_for(app), kind, app.name.clone(), app.icon.clone())
    }
}

/// The same for a file picked with "Browse…" (DLG-APP-04, TGT-06): a
/// `.desktop` file takes the name and icon of the app in `list` with that
/// ID, an executable its file name and no icon, as the service lists it.
/// `None` for an empty path or a `.desktop` file the list does not know.
#[must_use]
pub fn browse_info(path: &str, list: &AppList) -> Option<TargetInfo> {
    let target = browse_target(path)?;
    let id = target.get("custom").and_then(Value::as_str)?;
    if id.ends_with(".desktop") {
        let app = list.apps.iter().find(|app| app.id == id)?;
        let (name, icon, packaging) = (app.name.clone(), app.icon.clone(), app.packaging);
        return Some(TargetInfo {
            packaging,
            ..chosen(target, TargetKind::Custom, name, icon)
        });
    }
    let name = std::path::Path::new(id)
        .file_name()
        .map_or_else(|| id.to_owned(), |name| name.to_string_lossy().into_owned());
    Some(chosen(target, TargetKind::Custom, name, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, browser: bool) -> AppInfo {
        AppInfo {
            id: id.to_owned(),
            name: name.to_owned(),
            is_browser: browser,
            ..AppInfo::default()
        }
    }

    fn list() -> AppList {
        AppList {
            apps: vec![
                AppInfo {
                    keywords: vec!["chat".to_owned()],
                    packaging: Some(Packaging::Flatpak),
                    ..app("com.discordapp.Discord.desktop", "Discord", false)
                },
                app("firefox.desktop", "Firefox", true),
                app("slack.desktop", "Slack", false),
                app("thunderbird.desktop", "Thunderbird", false),
                app("chrome.desktop", "Chrome", true),
            ],
            recent_sources: vec!["slack.desktop".to_owned(), "thunderbird.desktop".to_owned()],
        }
    }

    fn shape(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row.kind {
                RowKind::Header => format!("# {}", row.label),
                RowKind::App => row.label.clone(),
            })
            .collect()
    }

    #[test]
    fn a_target_chooser_has_browsers_then_all_apps() {
        // DLG-APP-02: no Recent Sources for a target
        let rows = rows(&list(), "", false);
        assert_eq!(
            shape(&rows),
            [
                "# Browsers",
                "Chrome",
                "Firefox",
                "# All Apps",
                "Discord",
                "Slack",
                "Thunderbird"
            ]
        );
    }

    #[test]
    fn source_apps_start_with_recent_sources_in_order() {
        // DLG-APP-02: most recent first, listed once
        let rows = rows(&list(), "", true);
        assert_eq!(
            shape(&rows),
            [
                "# Recent Sources",
                "Slack",
                "Thunderbird",
                "# Browsers",
                "Chrome",
                "Firefox",
                "# All Apps",
                "Discord"
            ]
        );
    }

    #[test]
    fn search_filters_by_name_id_and_keywords() {
        // DLG-APP-01
        assert_eq!(
            shape(&rows(&list(), "fire", false)),
            ["# Browsers", "Firefox"]
        );
        assert_eq!(
            shape(&rows(&list(), "discordapp", false)),
            ["# All Apps", "Discord"]
        );
        assert_eq!(
            shape(&rows(&list(), "CHAT", false)),
            ["# All Apps", "Discord"]
        );
        assert!(rows(&list(), "zzz", false).is_empty());
    }

    #[test]
    fn every_word_of_the_search_must_match() {
        assert_eq!(
            shape(&rows(&list(), "chat disc", false)),
            ["# All Apps", "Discord"]
        );
        assert!(rows(&list(), "chat slack", false).is_empty());
    }

    #[test]
    fn packaging_shows_as_a_badge() {
        // DLG-APP-02
        assert_eq!(rows(&list(), "discord", false)[1].packaging, "Flatpak");
        assert_eq!(rows(&list(), "slack", false)[1].packaging, "");
    }

    #[test]
    fn a_chosen_browser_is_an_app_target_and_another_app_a_custom_one() {
        // TGT-06
        let apps = list().apps;
        assert_eq!(target_for(&apps[1]), json!({"app": "firefox.desktop"}));
        assert_eq!(target_for(&apps[2]), json!({"custom": "slack.desktop"}));
    }

    #[test]
    fn browse_turns_a_desktop_file_into_its_id_and_keeps_an_executable_path() {
        // DLG-APP-04
        assert_eq!(
            browse_target("/usr/share/applications/foo.desktop"),
            Some(json!({"custom": "foo.desktop"}))
        );
        assert_eq!(
            browse_target("/opt/tool/bin/tool"),
            Some(json!({"custom": "/opt/tool/bin/tool"}))
        );
        assert_eq!(browse_target("  "), None);
    }

    #[test]
    fn a_chosen_app_is_listed_as_the_service_would() {
        // TGT-06: a browser is an app target, any other app a custom one
        let apps = AppList {
            apps: vec![AppInfo {
                icon: Some("discord".to_owned()),
                packaging: Some(Packaging::Flatpak),
                ..app("discord.desktop", "Discord", false)
            }],
            ..AppList::default()
        };
        let info = chosen_info(&apps.apps[0]);
        assert_eq!(info.target, json!({"custom": "discord.desktop"}));
        assert_eq!(info.kind, TargetKind::Custom);
        assert_eq!(
            (info.name.as_str(), info.icon.as_deref(), info.packaging),
            ("Discord", Some("discord"), Some(Packaging::Flatpak))
        );
        assert!(!info.missing);
        let browser = chosen_info(&app("firefox.desktop", "Firefox", true));
        assert_eq!(browser.target, json!({"app": "firefox.desktop"}));
        assert_eq!(browser.kind, TargetKind::App);
    }

    #[test]
    fn browse_labels_a_desktop_file_from_the_list_and_an_executable_by_its_name() {
        // DLG-APP-04, TGT-06
        let list = AppList {
            apps: vec![AppInfo {
                icon: Some("/icons/foo.png".to_owned()),
                ..app("foo.desktop", "Foo", false)
            }],
            ..AppList::default()
        };
        let desktop = browse_info("/usr/share/applications/foo.desktop", &list).expect("known");
        assert_eq!(desktop.target, json!({"custom": "foo.desktop"}));
        assert_eq!(
            (desktop.name.as_str(), desktop.icon.as_deref()),
            ("Foo", Some("/icons/foo.png"))
        );
        let exe = browse_info("/opt/tool/bin/tool", &list).expect("executable");
        assert_eq!(exe.target, json!({"custom": "/opt/tool/bin/tool"}));
        assert_eq!((exe.name.as_str(), exe.icon.as_deref()), ("tool", None));
        assert_eq!(
            browse_info("/usr/share/applications/bar.desktop", &list),
            None
        );
        assert_eq!(browse_info("  ", &list), None);
    }
}
