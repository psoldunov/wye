//! The stand-in for the picker (PIPE-13) while no picker can be shown.
//!
//! A link that would show the picker opens in the most likely browser
//! instead, so links never go nowhere: the browser Wye replaced as the
//! default, then the first available browser the picker would show, then
//! the first installed web browser. Moved here from `wye open`, which keeps
//! its own copy for when the service cannot be reached.

use wye_api::Error;
use wye_core::{Availability as _, Chosen, Config, DesktopId, OpenOptions, Target};
use wye_desktop::Inventory;

use super::Result;
use super::link::{self, Activation, PickerNeeded};
use crate::context::ServiceContext;

/// Open the link `needed` describes without a picker.
pub(crate) async fn open_without_picker(
    ctx: &ServiceContext,
    needed: PickerNeeded,
    activation: &Activation,
) -> Result<()> {
    let plan = link::with_snapshot(ctx, move |snapshot| {
        let target = choose(
            snapshot.pipeline.config(),
            &snapshot.inventory,
            snapshot.previous_default.as_ref(),
        )?;
        let chosen = Chosen {
            target,
            options: OpenOptions::default(),
        };
        Some(link::plan(
            snapshot,
            &needed.resolution,
            &needed.request,
            Some(chosen),
        ))
    })
    .await?
    .ok_or_else(|| {
        Error::failed("the picker is not available yet and no web browser is installed")
    })?;
    tracing::info!(target = %plan.target, "the picker is not available yet; opening in the likeliest browser");
    link::open_plan(ctx, plan, activation).await
}

/// Where a picker-bound link opens for now, or `None` when no browser is
/// installed.
pub(crate) fn choose(
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
    use wye_desktop::{DesktopEntry, InstalledApp, XdgDirs};

    use super::*;

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
    fn prefers_the_previous_default() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            Some(&id("zeta.desktop")),
        );
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn skips_a_previous_default_that_is_gone() {
        let chosen = choose(
            &shown(&["beta.desktop"]),
            &inventory(),
            Some(&id("gone.desktop")),
        );
        assert_eq!(chosen, Some(target("beta.desktop")));
    }

    #[test]
    fn then_the_first_web_browser_by_name() {
        assert_eq!(
            choose(&Config::default(), &inventory(), None),
            Some(target("zeta.desktop"))
        );
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
        let chosen = choose(&config, &inventory, Some(&id("aaa-opener.desktop")));
        assert_eq!(chosen, Some(target("zeta.desktop")));
    }

    #[test]
    fn none_without_browsers() {
        let inventory = Inventory::from_apps(vec![app("e.desktop", "E", false)], Vec::new());
        assert_eq!(choose(&Config::default(), &inventory, None), None);
    }
}
