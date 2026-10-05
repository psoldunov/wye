//! Adopting browsers discovery finds for the first time into the shown list
//! (SHOWN-09, DISC-02). Pure: the service reads the state and the
//! configuration, calls [`adopt_new_browsers`] and applies the answer.

use crate::config::{Config, ShownEntry};
use crate::target::{DesktopId, Target};

use super::TargetCatalog;

/// What [`adopt_new_browsers`] decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adoption {
    /// The handlers discovery has now offered, to keep in the state: the
    /// ones seen before, then the new ones. Never pruned, so a browser the
    /// user hid stays hidden when it is uninstalled and installed again.
    pub seen: Vec<DesktopId>,
    /// The new `browsers.shown` list; `None` when it stays as it is.
    pub shown: Option<Vec<ShownEntry>>,
}

/// A browser found for the first time is appended to `browsers.shown`
/// (SHOWN-09), without a hotkey, so it appears in the picker and the tray
/// menu.
///
/// The candidates are the web-link handlers (`catalog.handlers`) except the
/// `excluded` apps, the own apps of web services, which are not browsers
/// (TGT-05). Only plain app targets are added, never profiles or private
/// windows. `seen` is the state's record of earlier offers, `None` before
/// the first scan: that scan only records the candidates, so a browser the
/// user unchecked earlier does not come back. An empty shown list is left
/// empty, because it already shows every installed browser (SHOWN-02).
/// The answer is idempotent: apply `seen` and `shown`, call again, and
/// nothing is left to change.
#[must_use]
pub fn adopt_new_browsers(
    config: &Config,
    catalog: &TargetCatalog,
    excluded: &[DesktopId],
    seen: Option<&[DesktopId]>,
) -> Adoption {
    let candidates: Vec<&DesktopId> = catalog
        .sorted_handlers()
        .into_iter()
        .map(|handler| &handler.app)
        .filter(|app| !excluded.contains(app))
        .collect();
    let Some(seen) = seen else {
        return Adoption {
            seen: candidates.into_iter().cloned().collect(),
            shown: None,
        };
    };
    let new: Vec<&DesktopId> = candidates
        .into_iter()
        .filter(|app| !seen.contains(app))
        .collect();
    let adopted = Adoption {
        seen: seen.iter().chain(new.iter().copied()).cloned().collect(),
        shown: None,
    };
    if config.browsers.shown.is_empty() {
        return adopted;
    }
    let appended: Vec<ShownEntry> = new
        .into_iter()
        .filter(|app| !is_listed(config, app))
        .map(|app| ShownEntry {
            target: Target::App(app.clone()),
            hotkey: None,
        })
        .collect();
    if appended.is_empty() {
        return adopted;
    }
    Adoption {
        shown: Some(
            config
                .browsers
                .shown
                .iter()
                .cloned()
                .chain(appended)
                .collect(),
        ),
        ..adopted
    }
}

fn is_listed(config: &Config, app: &DesktopId) -> bool {
    config
        .browsers
        .shown
        .iter()
        .any(|entry| matches!(&entry.target, Target::App(listed) if listed == app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target_menu::{HandlerEntry, ProfileEntry};

    fn id(name: &str) -> DesktopId {
        DesktopId::new(name).expect("valid desktop ID")
    }

    fn handler(app: &str, name: &str) -> HandlerEntry {
        HandlerEntry {
            app: id(app),
            name: name.to_owned(),
            icon: None,
            browser: true,
            private: true,
            new_window: true,
            profiles: vec![ProfileEntry {
                id: "Default".to_owned(),
                name: "Default".to_owned(),
                badge: None,
            }],
        }
    }

    fn catalog(handlers: &[(&str, &str)]) -> TargetCatalog {
        TargetCatalog {
            handlers: handlers
                .iter()
                .map(|(app, name)| handler(app, name))
                .collect(),
            ..TargetCatalog::default()
        }
    }

    fn config(shown: &[(&str, Option<&str>)]) -> Config {
        let mut config = Config::default();
        config.browsers.shown = shown
            .iter()
            .map(|(app, hotkey)| ShownEntry {
                target: Target::App(id(app)),
                hotkey: hotkey.map(str::to_owned),
            })
            .collect();
        config
    }

    fn ids(names: &[&str]) -> Vec<DesktopId> {
        names.iter().map(|name| id(name)).collect()
    }

    const TWO_BROWSERS: [(&str, &str); 2] = [
        ("firefox.desktop", "Firefox"),
        ("chromium.desktop", "Chromium"),
    ];

    // SHOWN-09: the first scan only records what is installed.
    #[test]
    fn the_first_scan_records_every_candidate_and_changes_nothing() {
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&TWO_BROWSERS),
            &[],
            None,
        );
        assert_eq!(adoption.seen, ids(&["chromium.desktop", "firefox.desktop"]));
        assert_eq!(adoption.shown, None);
    }

    #[test]
    fn a_new_browser_is_appended_after_the_users_entries() {
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", Some("f"))]),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&ids(&["firefox.desktop"])),
        );
        let shown = adoption.shown.expect("a new list");
        let apps: Vec<&Target> = shown.iter().map(|entry| &entry.target).collect();
        assert_eq!(
            apps,
            [
                &Target::App(id("firefox.desktop")),
                &Target::App(id("chromium.desktop")),
            ]
        );
        assert_eq!(shown[0].hotkey.as_deref(), Some("f"), "hotkey kept");
        assert_eq!(shown[1].hotkey, None);
        assert_eq!(adoption.seen, ids(&["firefox.desktop", "chromium.desktop"]));
    }

    #[test]
    fn several_new_browsers_come_in_name_order() {
        let handlers = [
            ("zen.desktop", "zen"),
            ("brave.desktop", "Brave"),
            ("opera.desktop", "Opera"),
            ("firefox.desktop", "Firefox"),
        ];
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&handlers),
            &[],
            Some(&ids(&["firefox.desktop"])),
        );
        let shown = adoption.shown.expect("a new list");
        let apps: Vec<&Target> = shown.iter().map(|entry| &entry.target).collect();
        assert_eq!(
            apps,
            [
                &Target::App(id("firefox.desktop")),
                &Target::App(id("brave.desktop")),
                &Target::App(id("opera.desktop")),
                &Target::App(id("zen.desktop")),
            ]
        );
    }

    // SHOWN-02: an empty list already shows every installed browser.
    #[test]
    fn an_empty_list_stays_empty_but_the_browser_is_recorded() {
        let adoption = adopt_new_browsers(
            &Config::default(),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&ids(&["firefox.desktop"])),
        );
        assert_eq!(adoption.shown, None);
        assert_eq!(adoption.seen, ids(&["firefox.desktop", "chromium.desktop"]));
    }

    // SHOWN-09: a browser the user unchecked stays out.
    #[test]
    fn a_seen_browser_the_user_removed_is_not_added_again() {
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&ids(&["firefox.desktop", "chromium.desktop"])),
        );
        assert_eq!(adoption.shown, None);
        assert_eq!(adoption.seen, ids(&["firefox.desktop", "chromium.desktop"]));
    }

    #[test]
    fn an_uninstalled_browser_stays_seen_so_a_reinstall_stays_hidden() {
        let seen = ids(&["firefox.desktop", "chromium.desktop"]);
        let gone = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&[("firefox.desktop", "Firefox")]),
            &[],
            Some(&seen),
        );
        assert_eq!(gone.seen, seen, "not pruned");
        let back = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&gone.seen),
        );
        assert_eq!(back.shown, None);
    }

    #[test]
    fn a_browser_already_listed_is_not_listed_twice() {
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", None), ("chromium.desktop", Some("c"))]),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&[]),
        );
        assert_eq!(adoption.shown, None);
        assert_eq!(adoption.seen, ids(&["chromium.desktop", "firefox.desktop"]));
    }

    // SHOWN-09: profiles and private windows are not added; the handler's
    // profile and private support does not matter.
    #[test]
    fn only_the_plain_app_target_is_added() {
        let adoption = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&TWO_BROWSERS),
            &[],
            Some(&ids(&["firefox.desktop"])),
        );
        let shown = adoption.shown.expect("a new list");
        assert!(
            shown
                .iter()
                .all(|entry| matches!(entry.target, Target::App(_)))
        );
    }

    // TGT-05: a service's own app is not a browser.
    #[test]
    fn excluded_apps_are_neither_adopted_nor_recorded() {
        let handlers = [("firefox.desktop", "Firefox"), ("slack.desktop", "Slack")];
        let excluded = ids(&["slack.desktop"]);
        let first = adopt_new_browsers(&config(&[]), &catalog(&handlers), &excluded, None);
        assert_eq!(first.seen, ids(&["firefox.desktop"]));
        let next = adopt_new_browsers(
            &config(&[("firefox.desktop", None)]),
            &catalog(&handlers),
            &excluded,
            Some(&first.seen),
        );
        assert_eq!(next.shown, None);
        assert_eq!(next.seen, ids(&["firefox.desktop"]));
    }

    #[test]
    fn a_second_call_with_the_returned_state_changes_nothing() {
        let handlers = [
            ("firefox.desktop", "Firefox"),
            ("chromium.desktop", "Chromium"),
            ("falkon.desktop", "Falkon"),
        ];
        let catalog = catalog(&handlers);
        let mut applied = config(&[("firefox.desktop", Some("f"))]);
        let first = adopt_new_browsers(&applied, &catalog, &[], Some(&ids(&["firefox.desktop"])));
        applied.browsers.shown = first.shown.clone().expect("a new list");
        let second = adopt_new_browsers(&applied, &catalog, &[], Some(&first.seen));
        assert_eq!(second.shown, None);
        assert_eq!(second.seen, first.seen);
    }
}
