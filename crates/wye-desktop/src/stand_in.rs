//! The picker stand-in (PIPE-13): where a link that needs the picker opens
//! when no picker can be shown (the UI host is unreachable, or `wye open`
//! routes without the service).
//!
//! The most likely browser: the one Wye replaced as the default, then the
//! first available browser the picker would show, then the first installed
//! web browser. Never an app that would send the link back to Wye (DEF-06).

use wye_core::{Availability as _, Config, DesktopId, Resolution, Target};

use crate::discovery::Inventory;

/// True when `resolution` would show the picker: its target, or a picker
/// held until the screen unlocks (PKS-07), which the stand-in opens at once.
#[must_use]
pub fn needed(resolution: &Resolution) -> bool {
    resolution.hold_until_unlock || !resolution.target.is_concrete()
}

/// Where a picker-bound link opens instead, or `None` when no web browser
/// is installed. `previous_default` is the browser Wye replaced (DEF-05).
#[must_use]
pub fn choose(
    config: &Config,
    inventory: &Inventory,
    previous_default: Option<&DesktopId>,
) -> Option<Target> {
    let previous = previous_default.map(|id| Target::App(id.clone()));
    let shown = config
        .browsers
        .shown
        .iter()
        .map(|entry| entry.target.clone());
    previous
        .into_iter()
        .chain(shown)
        .find(|target| target.is_concrete() && inventory.is_available(target))
        .or_else(|| {
            inventory
                .web_handlers()
                .first()
                .map(|app| Target::App(app.id().clone()))
        })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use wye_core::config::ShownEntry;

    use super::*;
    use crate::{DesktopEntry, InstalledApp, XdgDirs};

    fn xdg() -> XdgDirs {
        XdgDirs::from_lookup(|name| (name == "HOME").then(|| OsString::from("/nonexistent")))
            .expect("home")
    }

    fn app(id: &str, name: &str, web: bool) -> InstalledApp {
        app_running(id, name, web, "x")
    }

    fn app_running(id: &str, name: &str, web: bool, program: &str) -> InstalledApp {
        let mime = if web {
            "MimeType=x-scheme-handler/https;\n"
        } else {
            ""
        };
        let text =
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec={program} %u\n{mime}");
        let id = DesktopId::new(id).expect("id");
        let entry = DesktopEntry::parse(id, PathBuf::from("/x"), &text).expect("entry");
        InstalledApp::from_entry(entry, &xdg(), &mut Vec::new())
    }

    fn inventory() -> Inventory {
        Inventory::from_apps(
            vec![
                app("zeta.desktop", "Alpha", true),
                app("beta.desktop", "Beta", true),
                app("editor.desktop", "Editor", false),
            ],
            Vec::new(),
        )
    }

    fn id(value: &str) -> DesktopId {
        DesktopId::new(value).expect("id")
    }

    fn target(value: &str) -> Target {
        Target::App(id(value))
    }

    fn shown(ids: &[&str]) -> Config {
        let mut config = Config::default();
        config.browsers.shown = ids
            .iter()
            .map(|value| ShownEntry {
                target: target(value),
                hotkey: None,
            })
            .collect();
        config
    }

    #[test]
    fn pipe_13_prefers_the_previous_default() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            Some(&id("zeta.desktop")),
        );
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn pipe_13_skips_a_previous_default_that_is_gone() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            Some(&id("gone.desktop")),
        );
        assert_eq!(chosen, Some(target("beta.desktop")));
    }

    #[test]
    fn pipe_13_then_the_first_available_shown_browser() {
        let config = shown(&["gone.desktop", "beta.desktop"]);
        assert_eq!(
            choose(&config, &inventory(), None),
            Some(target("beta.desktop"))
        );
    }

    #[test]
    fn pipe_13_then_the_first_web_browser_by_name() {
        assert_eq!(
            choose(&Config::default(), &inventory(), None),
            Some(target("zeta.desktop"))
        );
    }

    #[test]
    fn def_06_never_chooses_an_app_that_loops_back() {
        let inventory = Inventory::from_apps(
            vec![
                app_running("aaa-opener.desktop", "Aaa", true, "xdg-open"),
                app_running("old-wye.desktop", "Old Wye", true, "/usr/bin/wye"),
                app("zeta.desktop", "Zeta", true),
            ],
            Vec::new(),
        );
        let config = shown(&["aaa-opener.desktop", "old-wye.desktop"]);
        let chosen = choose(&config, &inventory, Some(&id("aaa-opener.desktop")));
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn none_without_browsers() {
        let inventory = Inventory::from_apps(vec![app("e.desktop", "E", false)], Vec::new());
        assert_eq!(choose(&Config::default(), &inventory, None), None);
    }
}
