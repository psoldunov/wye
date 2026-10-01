//! The Browsers page (05-browsers.md): the primary and the alternative
//! browser, the alternative-browser key, the shown browsers sheet, and the
//! browser profiles' detection status. BRW-01 to BRW-06, in one card as on
//! KDE.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/BrowsersPage.qml.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::json;
use wye_api::targets::TargetKind;

use super::{Context, Page};
use crate::settings::menu::Surface;
use crate::settings::sheets::app_chooser::{AppChooser, Choice};
use crate::settings::sheets::shown_browsers::ShownBrowsersSheet;
use crate::settings::store::SettingsStore;
use crate::settings::sync::Action;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::target_row::TargetRow;
use crate::widgets::{group, help, links, row};

/// BRW-05: where the profile kinds are explained.
const PROFILES_SUBTITLE: &str = "Profiles of Chromium-based and Firefox-based browsers are found automatically. <a href=\"https://github.com/psoldunov/wye#browser-profiles\">Learn more</a>";

/// The Browsers page.
#[derive(Debug)]
pub struct BrowsersPage {
    page: adw::PreferencesPage,
    state: Rc<State>,
}

/// What the page's sheets and the self-test reach.
#[derive(Debug)]
struct State {
    context: Context,
    primary: TargetRow,
    alternative: TargetRow,
}

/// The widgets `show` updates.
#[derive(Debug, Clone)]
struct Widgets {
    alternative: adw::ComboRow,
    count: gtk::Label,
}

impl BrowsersPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("Browsers")
            .name("browsers")
            .build();
        let card = group::group("");

        // BRW-01
        let primary = TargetRow::new("Browser", "", Surface::Browsers);
        primary.bind(store, "browsers.primary", &json!({"picker": true}));
        card.add(primary.row());

        // BRW-02: the subtitle names the key set in BRW-03 (`show`).
        let alternative = TargetRow::new("Alternative browser", "", Surface::Browsers);
        alternative.bind(store, "browsers.alternative", &json!({"picker": true}));
        help::add_help(store, alternative.row(), "alternative-browser");
        card.add(alternative.row());

        // BRW-03
        let key = ModifierChooser::new();
        let key_row = key.add_row("Alternative browser key", "");
        key.bind(store, "browsers.alternative-key");
        card.add(&key_row);

        // BRW-04
        let shown = ButtonRow::new(
            "Shown browsers",
            "Browsers shown in the picker and the tray menu.",
            "Choose…",
        );
        // The whole row opens the sheet, not only its button.
        shown.activate_with_row();
        card.add(shown.row());

        // BRW-05, BRW-06: the count, then Rescan. Not a configuration
        // change: it works on a read-only file too.
        let profiles = row::action_row("Browser profiles", PROFILES_SUBTITLE);
        help::add_help(store, &profiles, "browser-profiles");
        let count = gtk::Label::builder().valign(gtk::Align::Center).build();
        count.add_css_class("dimmed");
        profiles.add_suffix(&count);
        let rescan = gtk::Button::builder()
            .label("Rescan")
            .valign(gtk::Align::Center)
            .build();
        profiles.add_suffix(&rescan);
        links::route_links(&profiles, context.link_opener());
        card.add(&profiles);
        page.add(&card);

        let state = Rc::new(State {
            context: context.clone(),
            primary,
            alternative,
        });
        connect(&state, &shown, &rescan);
        let widgets = Widgets {
            alternative: state.alternative.row().clone(),
            count,
        };
        show(store, &widgets);
        store.connect_changed(move |store| show(store, &widgets));
        Self { page, state }
    }
}

impl Page for BrowsersPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    /// `shown-browsers`, `shown-hotkey` (its first hotkey popup),
    /// `app-chooser` (over the shown browsers sheet; `app-chooser:<search>`
    /// types a search), `target-menu` and `alternative-menu` (a row's target
    /// menu open).
    fn open_sheet(&self, name: &str) -> bool {
        let state = &self.state;
        let (name, search) = name.split_once(':').unwrap_or((name, ""));
        match name {
            "shown-browsers" => {
                open_shown(state);
            }
            "shown-hotkey" => {
                if let Some(sheet) = open_shown(state) {
                    sheet.open_hotkey();
                }
            }
            "app-chooser" => {
                if let Some(sheet) = open_shown(state) {
                    sheet.add_app(search);
                }
            }
            "target-menu" => {
                adw::prelude::ActionRowExt::activate(state.primary.row());
            }
            "alternative-menu" => {
                adw::prelude::ActionRowExt::activate(state.alternative.row());
            }
            _ => return false,
        }
        true
    }
}

/// The buttons, and "Other…" of both target menus (TGT-06).
fn connect(state: &Rc<State>, shown: &ButtonRow, rescan: &gtk::Button) {
    let weak = Rc::downgrade(state);
    shown.button().connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(state) = weak.upgrade() {
                open_shown(&state);
            }
        }
    ));
    rescan.connect_clicked(glib::clone!(
        #[weak(rename_to = store)]
        state.context.store,
        move |_| store.act(Action::Rescan)
    ));
    for (row, path) in [
        (&state.primary, "browsers.primary"),
        (&state.alternative, "browsers.alternative"),
    ] {
        let weak = weak.clone();
        row.connect_other(move || {
            if let Some(state) = weak.upgrade() {
                choose_other(&state, path);
            }
        });
    }
}

/// SHOWN-01: the shown browsers sheet over the window.
fn open_shown(state: &State) -> Option<ShownBrowsersSheet> {
    let window = state.context.window()?;
    Some(ShownBrowsersSheet::open(&window, &state.context.store))
}

/// TGT-06: "Other…" chooses any app as the target at `path`.
fn choose_other(state: &State, path: &'static str) {
    let Some(window) = state.context.window() else {
        return;
    };
    let store = state.context.store.downgrade();
    AppChooser::open(
        &window,
        &state.context.store,
        Choice::Single,
        move |targets| {
            if let (Some(store), Some(target)) = (store.upgrade(), targets.first()) {
                store.set_target(path, target);
            }
        },
    );
}

/// Show what the store holds that no `bind` covers.
fn show(store: &SettingsStore, widgets: &Widgets) {
    // BRW-02 names the key BRW-03 sets, read as the help text reads it
    // (the configuration's default when the file sets none).
    let key = store.with_snapshot(|s| s.typed_config().browsers.alternative_key);
    let subtitle = if key.is_empty() {
        "Set a key below to open links in the alternative browser.".to_owned()
    } else {
        format!(
            "Hold {} while opening a link to open it in the alternative browser.",
            glib::markup_escape_text(&key.to_string())
        )
    };
    row::set_subtitle(&widgets.alternative, &subtitle);

    // BRW-06. Wye ships no translations, so each English form is its own
    // string.
    let count = store.with_snapshot(|s| {
        s.targets
            .targets
            .iter()
            .filter(|target| target.kind == TargetKind::Profile && !target.missing)
            .count()
    });
    let text = match count {
        0 => "No profiles found".to_owned(),
        1 => "1 profile found".to_owned(),
        n => format!("{n} profiles found"),
    };
    widgets.count.set_label(&text);
    widgets.count.set_visible(store.loaded());
}
