//! The Advanced page (10-advanced.md): URL expansion, the global transform
//! script, global keyboard shortcuts, history, the browser-extension
//! override and the frontend. ADV-01 to ADV-12.
//!
//! The URL Expansion sheet (DLG-EXP) is [`expansion_sheet`] over wye-ui's
//! Qt-free [`expansion`]; the Keyboard Shortcuts rows (ADV-05 to ADV-07,
//! KEY-40, KEY-41) are [`shortcut_rows`] over wye-ui's [`shortcuts`].
//!
//! KDE counterpart: crates/wye-ui/qml/settings/AdvancedPage.qml.

// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, so its nested `mod tests;` is found in `advanced/expansion/tests.rs`.
#[allow(
    dead_code,
    reason = "the file is shared whole; the sheet uses only part of it"
)]
mod expansion;
mod expansion_sheet;
mod shortcut_rows;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// A symlink, like `expansion`; its tests are inline.
#[allow(
    dead_code,
    reason = "the file is shared whole; the page uses only part of it"
)]
mod shortcuts;

/// What `shortcuts` labels a binding with (`super::record::labels`, KEY-03):
/// the kit's reading of a stored binding, so the rows read as the recorder
/// does. wye-ui's `record` reads Qt key events, so it is not shared.
mod record {
    pub fn labels(stored: &[String]) -> Vec<String> {
        stored
            .iter()
            .map(|binding| crate::widgets::shortcut::label(binding))
            .collect()
    }
}

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::{Context, Page};
use crate::settings::snapshot::Snapshot;
use crate::settings::store::SettingsStore;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::choice_row::ChoiceRow;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::{group, help, links, row, switch_row};
use expansion_sheet::ExpansionSheet;
use shortcut_rows::ShortcutRows;

/// ADV-12: the frontends, as `advanced.frontend` names them.
const FRONTENDS: [(&str, &str); 3] = [("auto", "Automatic"), ("kde", "KDE"), ("gnome", "GNOME")];

/// ADV-12, wording as on KDE.
const FRONTEND_SUBTITLE: &str = "Automatic uses GNOME on GNOME and KDE everywhere else. Applies to windows opened after the change.";

/// ADV-08: the note under the shortcuts; its links switch pages.
const KEYS_NOTE: &str = "Picker keys are on the <a href=\"page:picker\">Picker</a> page; the alternative browser key is on the <a href=\"page:browsers\">Browsers</a> page.";

/// The argument `ShowWindow("script-editor", …)` takes for the global
/// script (ADV-04, SCR-09).
const GLOBAL_SCRIPT: &str = r#"{"scope":"global"}"#;

/// The subtitle of a chooser the session cannot serve (KEY-06).
const UNAVAILABLE: &str = "Not available in this session";

/// Sheets `open_sheet` takes.
const EXPANSION_SHEET: &str = "expansion";
const HISTORY_CONFIRM: &str = "history-confirm";

/// The Advanced page.
#[derive(Debug)]
pub struct AdvancedPage {
    page: adw::PreferencesPage,
    dialogs: Rc<Dialogs>,
}

/// The sheet and the question this page opens over the window.
#[derive(Debug)]
struct Dialogs {
    context: Context,
    /// The URL Expansion sheet, built the first time it opens.
    expansion: OnceCell<ExpansionSheet>,
    /// "Delete Stored History?" while it is asked.
    history: RefCell<Option<adw::AlertDialog>>,
}

impl Dialogs {
    /// ADV-02: the URL Expansion sheet.
    fn open_expansion(&self) {
        self.close_history();
        if let Some(window) = self.context.window() {
            self.expansion
                .get_or_init(|| ExpansionSheet::new(&self.context.store))
                .present(&window);
        }
    }

    /// ADV-09: turning history off asks whether to delete what is stored.
    fn ask_history(&self) {
        let Some(window) = self.context.window() else {
            return;
        };
        self.close_all();
        let dialog = adw::AlertDialog::builder()
            .heading("Delete Stored History?")
            .body("History is off now. Delete the links Wye stored while it was on?")
            .close_response("keep")
            .default_response("keep")
            .build();
        dialog.add_responses(&[("keep", "Keep History"), ("delete", "Delete History")]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        let store = self.context.store.downgrade();
        dialog.connect_response(None, move |_, response| {
            if let Some(store) = store.upgrade()
                && response == "delete"
            {
                store.call(|proxy| async move { proxy.clear_history().await });
            }
        });
        dialog.present(Some(&window));
        self.history.replace(Some(dialog));
    }

    fn close_history(&self) {
        let asked = self.history.borrow_mut().take();
        if let Some(dialog) = asked {
            dialog.force_close();
        }
    }

    fn close_all(&self) {
        self.close_history();
        if let Some(sheet) = self.expansion.get() {
            sheet.close();
        }
    }
}

impl AdvancedPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("Advanced")
            .name("advanced")
            .build();
        let dialogs = Rc::new(Dialogs {
            context: context.clone(),
            expansion: OnceCell::new(),
            history: RefCell::new(None),
        });

        page.add(&url_expansion(store, &dialogs));
        page.add(&url_transformation(store));
        page.add(&keyboard_shortcuts(store, &page));
        page.add(&history(store, &dialogs));
        page.add(&miscellaneous(store));

        // ADV-12
        let interface = group::group("Interface");
        let frontend = ChoiceRow::new("Frontend", FRONTEND_SUBTITLE, &FRONTENDS);
        frontend.bind(store, "advanced.frontend", "auto");
        interface.add(frontend.row());
        page.add(&interface);

        // What this page opened over the window goes when the page does (a
        // request that switches pages, the window hidden).
        let open = Rc::clone(&dialogs);
        page.connect_unmap(move |_| open.close_all());
        Self { page, dialogs }
    }
}

impl Page for AdvancedPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn open_sheet(&self, name: &str) -> bool {
        match name {
            EXPANSION_SHEET => self.dialogs.open_expansion(),
            HISTORY_CONFIRM => self.dialogs.ask_history(),
            _ => return false,
        }
        true
    }
}

/// ADV-01, ADV-02: expansion on or off, and Configure… for the sheet.
fn url_expansion(store: &SettingsStore, dialogs: &Rc<Dialogs>) -> adw::PreferencesGroup {
    let group = group::group("URL Expansion");
    let expand = ButtonRow::new("Expand redirect and short URLs", "", "Configure…").add_switch();
    expand.bind_switch(store, "advanced.expand-urls", true, false);
    let open = Rc::downgrade(dialogs);
    expand.button().connect_clicked(move |_| {
        if let Some(dialogs) = open.upgrade() {
            dialogs.open_expansion();
        }
    });
    group.add(expand.row());
    group
}

/// ADV-03, ADV-04: the global script, and Edit Script… for the editor.
fn url_transformation(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("URL Transformation");
    let transform = ButtonRow::new(
        "Transform all URLs before matching rules",
        "Runs after URL expansion and tracking removal.",
        "Edit Script…",
    )
    .add_switch();
    // SCR-09: turning the transform on with no script opens the editor.
    // Connected before the bind, so it still sees the old value.
    if let Some(switch) = transform.switch() {
        switch.connect_active_notify(glib::clone!(
            #[weak]
            store,
            move |switch| {
                if switch.is_active() && !store.bool_value("advanced.transform", false) {
                    let shown = store.downgrade();
                    store.script_exists("global", move |exists| {
                        if let Some(store) = shown.upgrade()
                            && !exists
                        {
                            store.show_window("script-editor", GLOBAL_SCRIPT.to_owned());
                        }
                    });
                }
            }
        ));
    }
    transform.bind_switch(store, "advanced.transform", false, false);
    transform.button().connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.show_window("script-editor", GLOBAL_SCRIPT.to_owned())
    ));
    group.add(transform.row());
    group
}

/// ADV-05 to ADV-08: one row per global shortcut, as the session allows,
/// and the note pointing to the other keys.
fn keyboard_shortcuts(store: &SettingsStore, page: &adw::PreferencesPage) -> adw::PreferencesGroup {
    let rows = ShortcutRows::new(store);
    let group = rows.group().clone();
    let note = links::link_label(
        KEYS_NOTE,
        glib::clone!(
            #[weak]
            page,
            #[weak]
            store,
            move |uri| match uri.strip_prefix("page:") {
                Some(id) => show_page(&page, id),
                None => store.open_link(uri),
            }
        ),
    );
    // Dimmed text, but links in the accent colour (`wye-group-note`).
    note.remove_css_class("dimmed");
    note.add_css_class("wye-group-note");
    group.add(&note);

    // The rows follow the configuration when the mechanism reports
    // nothing, so they are read again when it changes; a fixture changes
    // the mechanism too.
    let seen: Rc<RefCell<Option<Option<Value>>>> = Rc::default();
    let load = move |store: &SettingsStore| {
        let now = store.value("shortcuts");
        if store.offline() || seen.borrow().as_ref() != Some(&now) {
            seen.replace(Some(now));
            rows.load(store);
        }
    };
    load(store);
    store.connect_changed(load);
    group
}

/// ADV-09: history on or off, Show… for the History window; turning it off
/// asks whether to delete what is stored.
fn history(store: &SettingsStore, dialogs: &Rc<Dialogs>) -> adw::PreferencesGroup {
    let group = group::group("History");
    let history =
        ButtonRow::new("Store history of the last 100 opened links", "", "Show…").add_switch();
    if let Some(switch) = history.switch() {
        // Connected before the bind, so it still sees the old value: only
        // the user's flip asks, not a value the store pushes.
        let ask = Rc::downgrade(dialogs);
        switch.connect_active_notify(glib::clone!(
            #[weak]
            store,
            move |switch| {
                if !switch.is_active()
                    && store.bool_value("advanced.history", false)
                    && let Some(dialogs) = ask.upgrade()
                {
                    dialogs.ask_history();
                }
            }
        ));
    }
    history.bind_switch(store, "advanced.history", false, false);
    history.button().connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.show_window("history", String::new())
    ));
    group.add(history.row());
    group
}

/// ADV-10, ADV-11: the extension override and the key that skips it.
fn miscellaneous(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("Miscellaneous");
    let force = switch_row::switch_row("Force show picker when opening from browser extension", "");
    switch_row::bind(store, &force, "advanced.force-picker-from-extension", true);
    group.add(&force);

    let chooser = ModifierChooser::new();
    let bypass = chooser.add_row("Bypass key", "");
    help::add_help(store, &bypass, "bypass-key");
    group.add(&bypass);
    chooser.connect_changed(glib::clone!(
        #[weak]
        store,
        move |names| {
            if names != bypass_names(&store).as_slice() {
                store.set_modifiers("advanced.bypass-key", names);
            }
        }
    ));
    row::follow_writable(store, chooser.widget(), held_keys);

    let show = glib::clone!(
        #[weak]
        force,
        #[weak]
        bypass,
        move |store: &SettingsStore| {
            let names = bypass_names(store);
            chooser.set_pressed(&names);
            let key = if names.is_empty() {
                "the bypass key".to_owned()
            } else {
                names.join("+")
            };
            row::set_subtitle(
                &force,
                &format!("When opening, hold {key} to not force show the picker."),
            );
            // KEY-06
            let available = held_keys(store);
            row::set_unavailable(&bypass, !available);
            bypass.set_subtitle(if available { "" } else { UNAVAILABLE });
        }
    );
    show(store);
    store.connect_changed(show);
    group
}

/// Whether the session reports held modifiers (KEY-06).
fn held_keys(store: &SettingsStore) -> bool {
    store.with_snapshot(Snapshot::held_keys_available)
}

/// ADV-11: the bypass key, by name (Alt when the file sets none).
fn bypass_names(store: &SettingsStore) -> Vec<String> {
    store.with_snapshot(|s| {
        s.typed_config()
            .advanced
            .bypass_key
            .iter()
            .map(|modifier| modifier.name().to_owned())
            .collect()
    })
}

/// ADV-08: switch the window to page `id`; the window's switcher follows
/// (title, last page).
fn show_page(page: &adw::PreferencesPage, id: &str) {
    if let Some(stack) = page
        .ancestor(adw::ViewStack::static_type())
        .and_downcast::<adw::ViewStack>()
    {
        stack.set_visible_child_name(id);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn adv_12_frontends_match_the_configuration() {
        for (value, _) in FRONTENDS {
            let parsed: Result<wye_core::config::Frontend, _> =
                serde_json::from_value(Value::from(value));
            assert!(parsed.is_ok(), "{value} is not a frontend");
        }
        let labels: Vec<&str> = FRONTENDS.iter().map(|(_, label)| *label).collect();
        assert_eq!(labels, ["Automatic", "KDE", "GNOME"]);
    }

    #[test]
    fn adv_11_the_bypass_key_defaults_to_alt() {
        let store = SettingsStore::new();
        store
            .load_fixture(&json!({"config": {}, "status": {"config": {"writable": true}}}))
            .expect("a fixture");
        assert_eq!(bypass_names(&store), ["Alt"]);
    }

    #[test]
    fn key_03_rows_label_bindings_as_the_recorder_does() {
        assert_eq!(
            record::labels(&["Ctrl+Shift+o".to_owned()]),
            ["Ctrl+Shift+O"]
        );
    }
}
