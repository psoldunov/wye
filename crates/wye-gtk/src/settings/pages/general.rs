//! The General page (04-general.md): default-browser status, startup
//! behaviour, the tray icon, and the callout about links Wye cannot
//! intercept. GEN-01 to GEN-05.
//!
//! The reference page: every other page follows its shape. Build the
//! groups top to bottom from the widget kit, bind each configuration
//! control to its key with the kit's `bind`, and put what the store cannot
//! bind (a status, a value managed elsewhere) in one `show` function run on
//! every store change.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/GeneralPage.qml.

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::{Context, Page};
use crate::settings::store::SettingsStore;
use crate::settings::sync::Action;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::callout::Callout;
use crate::widgets::choice_row::ChoiceRow;
use crate::widgets::row::{self, LeadingIcon, Tint};
use crate::widgets::{group, switch_row};

/// GEN-02: the tray icon's values and their labels; the stored value never
/// shows.
const TRAY_ICONS: [(&str, &str); 2] = [("primary-browser", "Primary Browser"), ("wye", "Wye")];

/// GEN-04, wording as on KDE.
const LINKS_CALLOUT: &str = "<b>Wye cannot handle links clicked inside a browser.</b> You can either use the browser extension (see the website for more info), or copy the link and then choose “Open URL from Clipboard” in the Wye menu.";

/// The General page.
#[derive(Debug)]
pub struct GeneralPage {
    page: adw::PreferencesPage,
}

/// The widgets `show` updates.
#[derive(Debug, Clone)]
struct Widgets {
    status: ButtonRow,
    glyph: LeadingIcon,
    login: adw::SwitchRow,
}

impl GeneralPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("General")
            .name("general")
            .build();

        // GEN-05: the default-browser status and switch come first.
        let default_browser = group::group("Default Browser");
        let status = ButtonRow::new("", "", "Make Default");
        let glyph = row::add_leading_icon(status.row());
        default_browser.add(status.row());
        let local_html = switch_row::switch_row("Also open local HTML files", "");
        switch_row::bind(store, &local_html, "general.open-local-html", false);
        default_browser.add(&local_html);
        page.add(&default_browser);

        // GEN-01
        let startup = group::group("Startup");
        let login = switch_row::switch_row("Launch at login", "");
        startup.add(&login);
        page.add(&startup);

        // GEN-02, GEN-03
        let tray = group::group("Tray");
        let tray_icon = ChoiceRow::new("Tray icon", "", &TRAY_ICONS);
        tray_icon.bind(store, "general.tray-icon", "primary-browser");
        tray.add(tray_icon.row());
        let show_tray = switch_row::switch_row("Show tray icon", "");
        switch_row::bind(store, &show_tray, "general.show-tray-icon", true);
        tray.add(&show_tray);
        page.add(&tray);

        // GEN-04
        let callout = Callout::new(store, "general-links", "", LINKS_CALLOUT);
        page.add(callout.group());

        let widgets = Widgets {
            status,
            glyph,
            login,
        };
        connect(store, &widgets);
        show(store, &widgets);
        store.connect_changed(glib::clone!(
            #[strong]
            widgets,
            move |store| show(store, &widgets)
        ));
        Self { page }
    }
}

impl Page for GeneralPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
}

/// The two controls `show` sets, wired to the store.
fn connect(store: &SettingsStore, widgets: &Widgets) {
    // GEN-05: Make Default, or Stop Being Default (restores the previous
    // browser, DEF-05). Not a configuration change: works on a read-only
    // file too.
    widgets.status.button().connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| {
            let is_default = store.with_snapshot(|s| s.status.default_browser.is_default);
            store.act(if is_default {
                Action::StopBeingDefault
            } else {
                Action::MakeDefault
            });
        }
    ));
    // GEN-01: saved unless the Nix modules decide it (the switch is
    // insensitive then, so only a user flip on an editable file gets here).
    widgets.login.connect_active_notify(glib::clone!(
        #[weak]
        store,
        move |login| {
            let managed = store.with_snapshot(|s| s.status.login_managed);
            if !managed && login.is_active() != store.bool_value("general.launch-at-login", true) {
                store.set_value("general.launch-at-login", Value::Bool(login.is_active()));
            }
        }
    ));
}

/// Show what the store holds that no `bind` covers.
fn show(store: &SettingsStore, widgets: &Widgets) {
    let (status, writable) = store.with_snapshot(|s| (s.status.clone(), s.writable()));

    // GEN-05, ONB-10: a plain glyph, then the text; the button changes it.
    let browser = &status.default_browser;
    let row = widgets.status.row();
    if browser.is_default {
        row.set_title("Wye is your default browser");
        row.set_subtitle("");
        widgets.glyph.set("object-select-symbolic", Tint::Success);
        widgets.status.set_label("Stop Being Default");
    } else {
        row.set_title("Wye is not your default browser");
        let current = browser.current.as_ref().map(|app| app.name.as_str());
        let subtitle = current
            .filter(|name| !name.is_empty())
            .map_or_else(String::new, |name| {
                format!(
                    "Your default browser is {}.",
                    glib::markup_escape_text(name)
                )
            });
        row.set_subtitle(&subtitle);
        widgets.glyph.set("dialog-warning-symbolic", Tint::Warning);
        widgets.status.set_label("Make Default");
    }
    widgets.status.set_suggested(!browser.is_default);
    widgets.status.button().set_sensitive(store.loaded());

    // GEN-01: the Nix modules decide login start (`loginManagedOn`); the
    // switch shows what they set and changes nothing.
    let login = &widgets.login;
    let on = if status.login_managed {
        status.login_managed_on
    } else {
        store.bool_value("general.launch-at-login", true)
    };
    if login.is_active() != on {
        login.set_active(on);
    }
    let note = match (status.login_managed, status.login_managed_on) {
        (false, _) => "",
        (true, true) => "Your Nix configuration starts Wye at login. Change it there.",
        (true, false) => "Your Nix configuration does not start Wye at login. Change it there.",
    };
    login.set_subtitle(note);
    row::set_disabled(login, status.login_managed || !writable);
}
