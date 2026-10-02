//! The seven Settings pages (SET-02), in order: General, Browsers, Apps,
//! Picker, Rules, Extras, Advanced. Each is an `AdwPreferencesPage` built in
//! its own module from the widget kit (`crate::widgets`) and the window's
//! [`SettingsStore`].
//!
//! To build a page: add `pages/<id>.rs` with a type implementing [`Page`],
//! construct it in [`build`], and add its cases to
//! `fixtures/settings.json`. `general.rs` is the reference.

pub mod advanced;
pub mod apps;
pub mod browsers;
pub mod extras;
pub mod general;
pub mod picker;
pub mod placeholder;
pub mod rules;

use adw::prelude::*;
use gtk::glib;

use crate::settings::store::SettingsStore;

/// A page's place in the switcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    /// The name `ShowWindow("settings", id)` and the UI state use.
    pub id: &'static str,
    /// The switcher label and the window title (SET-01).
    pub title: &'static str,
    /// The switcher icon (SET-02): Adwaita's where it has a faithful one,
    /// else Wye's own from the `GResource` (`data/icons`).
    pub icon: &'static str,
}

/// Every page, in switcher order (SET-02).
pub const PAGES: [PageInfo; 7] = [
    PageInfo {
        id: "general",
        title: "General",
        icon: "applications-system-symbolic",
    },
    PageInfo {
        id: "browsers",
        title: "Browsers",
        icon: "wye-browsers-symbolic",
    },
    PageInfo {
        id: "apps",
        title: "Apps",
        icon: "wye-apps-symbolic",
    },
    PageInfo {
        id: "picker",
        title: "Picker",
        icon: "view-list-bullet-symbolic",
    },
    PageInfo {
        id: "rules",
        title: "Rules",
        icon: "wye-rules-symbolic",
    },
    PageInfo {
        id: "extras",
        title: "Extras",
        icon: "wye-extras-symbolic",
    },
    PageInfo {
        id: "advanced",
        title: "Advanced",
        icon: "wye-advanced-symbolic",
    },
];

/// The page called `id`.
#[must_use]
pub fn info(id: &str) -> Option<&'static PageInfo> {
    PAGES.iter().find(|page| page.id == id)
}

/// What every page gets: the store, and the window for dialogs and sheets.
#[derive(Debug, Clone)]
pub struct Context {
    pub store: SettingsStore,
    window: glib::WeakRef<adw::ApplicationWindow>,
}

impl Context {
    /// A context for pages of `window`.
    #[must_use]
    pub fn new(store: SettingsStore, window: &adw::ApplicationWindow) -> Self {
        Self {
            store,
            window: window.downgrade(),
        }
    }

    /// The Settings window, to present a sheet (`Sheet::present`) or a
    /// dialog over; `None` while it is being destroyed.
    #[must_use]
    pub fn window(&self) -> Option<adw::ApplicationWindow> {
        self.window.upgrade()
    }

    /// Open `url` through Wye (BLK-17), for `widgets::links::route_links`.
    pub fn link_opener(&self) -> impl Fn(&str) + 'static {
        let store = self.store.downgrade();
        move |url| {
            if let Some(store) = store.upgrade() {
                store.open_link(url);
            }
        }
    }
}

/// One page of the window.
pub trait Page {
    /// The page, added to the window's view stack once.
    fn widget(&self) -> &adw::PreferencesPage;

    /// Open the sheet called `name` (a self-test fixture's `sheet`); false
    /// when this page has no such sheet.
    fn open_sheet(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// A `ShowWindow` for this page that is not a plain page switch: the
    /// Rules page gets `rule-editor` (with its prefill) and `test-rules`.
    fn request(&self, key: &str, argument: &str) {
        tracing::debug!(%key, %argument, "this page takes no requests");
    }
}

/// Build the page `info` describes.
#[must_use]
pub fn build(info: &'static PageInfo, context: &Context) -> Box<dyn Page> {
    match info.id {
        "general" => Box::new(general::GeneralPage::new(context)),
        "browsers" => Box::new(browsers::BrowsersPage::new(context)),
        "apps" => Box::new(apps::AppsPage::new(context)),
        "rules" => Box::new(rules::RulesPage::new(context)),
        "picker" => Box::new(picker::PickerPage::new(context)),
        "extras" => Box::new(extras::ExtrasPage::new(context)),
        "advanced" => Box::new(advanced::AdvancedPage::new(context)),
        _ => Box::new(placeholder::PlaceholderPage::new(info)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_02_seven_pages_in_order() {
        let ids: Vec<&str> = PAGES.iter().map(|page| page.id).collect();
        assert_eq!(
            ids,
            [
                "general", "browsers", "apps", "picker", "rules", "extras", "advanced"
            ]
        );
        assert!(PAGES.iter().all(|page| page.icon.ends_with("-symbolic")));
        assert_eq!(info("rules").map(|page| page.title), Some("Rules"));
        assert_eq!(info("Rules"), None);
    }

    #[test]
    fn set_02_wye_icons_are_in_the_resource() {
        let resources = include_str!("../../../data/resources.gresource.xml");
        for page in PAGES.iter().filter(|page| page.icon.starts_with("wye-")) {
            let file = format!("icons/scalable/actions/{}.svg", page.icon);
            assert!(resources.contains(&file), "{file} is not in the GResource");
        }
    }
}
