//! The interim stand-in for the picker (PIPE-13).
//!
//! The picker window does not exist yet. Until it does, a link that would
//! show it opens in the most likely browser instead, so links never go
//! nowhere: the browser Wye replaced as the default, then the first
//! available browser the picker would show, then the first installed web
//! browser.

use wye_core::{Availability, Config, Resolution, Target};
use wye_desktop::Inventory;

use crate::state::State;

/// True when the link would show the picker: its target, or a picker held
/// until the screen unlocks (PKS-07), which the stand-in opens right away.
pub fn needed(resolution: &Resolution) -> bool {
    resolution.hold_until_unlock || !resolution.target.is_concrete()
}

/// Where a picker-bound link opens for now, or `None` when no browser is
/// installed.
pub fn choose(config: &Config, inventory: &Inventory, state: &State) -> Option<Target> {
    let previous = state
        .previous_default_browser
        .iter()
        .map(|id| Target::App(id.clone()));
    let shown = config
        .browsers
        .shown
        .iter()
        .map(|entry| entry.target.clone());
    previous
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

    use wye_core::DesktopId;
    use wye_core::config::ShownEntry;
    use wye_desktop::{DesktopEntry, InstalledApp, XdgDirs};

    use super::*;

    fn xdg() -> XdgDirs {
        XdgDirs::from_lookup(|name| (name == "HOME").then(|| OsString::from("/nonexistent")))
            .unwrap()
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
        let id = DesktopId::new(id).unwrap();
        let entry = DesktopEntry::parse(id, PathBuf::from("/x"), &text).unwrap();
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

    fn target(id: &str) -> Target {
        Target::App(DesktopId::new(id).unwrap())
    }

    fn shown(ids: &[&str]) -> Config {
        let mut config = Config::default();
        config.browsers.shown = ids
            .iter()
            .map(|id| ShownEntry {
                target: target(id),
                hotkey: None,
            })
            .collect();
        config
    }

    fn remembered(id: &str) -> State {
        State {
            previous_default_browser: Some(DesktopId::new(id).unwrap()),
        }
    }

    #[test]
    fn prefers_the_previous_default() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            &remembered("zeta.desktop"),
        );
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn skips_a_previous_default_that_is_gone() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            &remembered("gone.desktop"),
        );
        assert_eq!(chosen, Some(target("beta.desktop")));
    }

    #[test]
    fn then_the_first_available_shown_browser() {
        let config = shown(&["gone.desktop", "beta.desktop"]);
        let chosen = choose(&config, &inventory(), &State::default());
        assert_eq!(chosen, Some(target("beta.desktop")));
    }

    #[test]
    fn then_the_first_web_browser_by_name() {
        let chosen = choose(&Config::default(), &inventory(), &State::default());
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn never_chooses_an_app_that_loops_back() {
        // DEF-06: an entry running xdg-open or wye would send the link back.
        let inventory = Inventory::from_apps(
            vec![
                app_running("aaa-opener.desktop", "Aaa", true, "xdg-open"),
                app_running("old-wye.desktop", "Old Wye", true, "/usr/bin/wye"),
                app("zeta.desktop", "Zeta", true),
            ],
            Vec::new(),
        );
        let config = shown(&["aaa-opener.desktop", "old-wye.desktop"]);
        let chosen = choose(&config, &inventory, &remembered("aaa-opener.desktop"));
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn none_without_browsers() {
        let inventory = Inventory::from_apps(vec![app("e.desktop", "E", false)], Vec::new());
        assert_eq!(
            choose(&Config::default(), &inventory, &State::default()),
            None
        );
    }
}
