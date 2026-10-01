//! The URL Expansion sheet (17-dialogs.md, DLG-EXP-01 to DLG-EXP-05),
//! opened by Configure… on the Advanced page: which redirect wrappers Wye
//! unwraps on this computer, which short-link services it asks where a link
//! leads (and "+" for a domain of your own), and the limits: timeout,
//! maximum redirects, and whether to notify when a link cannot be expanded.
//! Changes apply at once; Done closes the sheet.
//!
//! The list is the service's (`GetExpansionCatalogue`); only what differs
//! from it is stored (`advanced.expansion`), and the rows come from both
//! through wye-ui's [`super::expansion`].
//!
//! KDE counterpart: crates/wye-ui/qml/settings/ExpansionSheet.qml.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;
use wye_core::config::ExpansionSettings;

use super::expansion::{self, Catalogue, Kind};
use crate::settings::store::SettingsStore;
use crate::widgets::callout::Callout;
use crate::widgets::section::AddSection;
use crate::widgets::sheet::Sheet;
use crate::widgets::{group, row, switch_row};

/// The callout at the top, as on KDE.
const INFO: &str = "Redirect wrappers are unwrapped on your computer. Short links need one request to the short-link service to find where they lead.";

/// DLG-EXP-04: the timeout moves in half seconds.
const TIMEOUT_STEP_S: f64 = 0.5;

/// The entry row's title while it shows no error.
const DOMAIN_TITLE: &str = "Short-link domain";

/// The URL Expansion sheet. Built once per window and presented again each
/// time; it follows the store while closed.
#[derive(Debug, Clone)]
pub struct ExpansionSheet {
    sheet: Sheet,
    open: Rc<Cell<bool>>,
    store: glib::WeakRef<SettingsStore>,
    lists: Rc<Lists>,
}

/// The two lists and what they show.
#[derive(Debug)]
struct Lists {
    catalogue: RefCell<Option<Catalogue>>,
    wrappers: adw::PreferencesGroup,
    short_links: AddSection,
    /// The entry that adds a domain, at the end of the short links.
    entry: adw::EntryRow,
    /// The rows shown now, by ID, in order.
    wrapper_rows: RefCell<Vec<(String, adw::SwitchRow)>>,
    link_rows: RefCell<Vec<(String, adw::SwitchRow)>>,
}

impl ExpansionSheet {
    /// The sheet on `store`; show it with [`Self::present`].
    #[must_use]
    pub fn new(store: &SettingsStore) -> Self {
        let sheet = Sheet::new("URL Expansion", "Done");
        sheet.cancel().set_visible(false);
        sheet.set_valid(true);
        sheet.connect_primary(|| true);
        let page = sheet.page();
        page.add(Callout::new(store, "expansion-info", "", INFO).group());

        // DLG-EXP-01
        let wrappers = group::group("Redirect Wrappers");
        page.add(&wrappers);
        // DLG-EXP-02
        let short_links = AddSection::new("Short Links", "", "No short-link domains");
        let add = short_links.add_button();
        add.set_tooltip_text(Some("Add Domain"));
        add.update_property(&[gtk::accessible::Property::Label("Add Domain")]);
        row::follow_writable(store, add, |_| true);
        page.add(short_links.group());
        let entry = domain_entry();
        short_links.add(&entry);
        // DLG-EXP-04
        page.add(&behaviour(store));

        let lists = Rc::new(Lists {
            catalogue: RefCell::new(None),
            wrappers,
            short_links,
            entry,
            wrapper_rows: RefCell::default(),
            link_rows: RefCell::default(),
        });
        connect_adding(store, &lists);
        let shown = Rc::downgrade(&lists);
        store.connect_changed(move |store| {
            if let Some(lists) = shown.upgrade() {
                lists.show(store);
            }
        });
        let open = Rc::new(Cell::new(false));
        let closed = Rc::clone(&open);
        sheet.dialog().connect_closed(move |_| closed.set(false));
        Self {
            sheet,
            open,
            store: store.downgrade(),
            lists,
        }
    }

    /// Read the catalogue again and show the sheet over `parent`'s window.
    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.close();
        self.lists.cancel_adding();
        if let Some(store) = self.store.upgrade() {
            let lists = Rc::downgrade(&self.lists);
            let shown = store.downgrade();
            store.expansion_catalogue(move |wire| {
                let (Some(lists), Some(store)) = (lists.upgrade(), shown.upgrade()) else {
                    return;
                };
                let custom = settings(&store).custom_short_links;
                lists
                    .catalogue
                    .replace(Some(Catalogue::from_wire(&wire, &custom)));
                lists.show(&store);
            });
        }
        self.sheet.page().scroll_to_top();
        self.sheet.present(parent);
        self.open.set(true);
    }

    /// Close the sheet if it is open.
    pub fn close(&self) {
        if self.open.replace(false) {
            self.sheet.dialog().force_close();
        }
    }
}

impl Lists {
    /// Show the rows the catalogue and the configuration make now.
    fn show(&self, store: &SettingsStore) {
        let Some(catalogue) = self.catalogue.borrow().clone() else {
            return;
        };
        let rows = expansion::rows(&catalogue, &settings(store));
        let (wrappers, links): (Vec<_>, Vec<_>) =
            rows.into_iter().partition(|row| row.kind == Kind::Wrapper);
        sync(
            store,
            &self.wrapper_rows,
            &wrappers,
            |row| self.wrappers.add(row),
            |row| {
                self.wrappers.remove(row);
            },
        );
        sync(
            store,
            &self.link_rows,
            &links,
            |row| self.add_link(row),
            |row| {
                self.short_links.remove(row);
            },
        );
    }

    /// A short-link row goes before the entry, which stays last.
    fn add_link(&self, row: &adw::SwitchRow) {
        let entry_shown = self.entry.parent().is_some();
        if entry_shown {
            self.short_links.remove(&self.entry);
        }
        self.short_links.add(row);
        if entry_shown {
            self.short_links.add(&self.entry);
        }
    }

    fn cancel_adding(&self) {
        self.entry.set_text("");
        self.entry.set_visible(false);
        show_error(&self.entry, "");
    }
}

/// Make `shown` match `rows`: the same IDs in the same order only update
/// their switches (so a click never rebuilds the row under the pointer);
/// anything else builds the list again.
fn sync(
    store: &SettingsStore,
    shown: &RefCell<Vec<(String, adw::SwitchRow)>>,
    rows: &[expansion::Row],
    add: impl Fn(&adw::SwitchRow),
    remove: impl Fn(&adw::SwitchRow),
) {
    let same = {
        let current = shown.borrow();
        current.len() == rows.len() && current.iter().zip(rows).all(|((id, _), row)| *id == row.id)
    };
    if same {
        for ((_, switch), row) in shown.borrow().iter().zip(rows) {
            if switch.is_active() != row.enabled {
                switch.set_active(row.enabled);
            }
        }
        return;
    }
    for (_, old) in shown.borrow_mut().drain(..) {
        remove(&old);
    }
    let built: Vec<(String, adw::SwitchRow)> = rows
        .iter()
        .map(|row| (row.id.clone(), entry_row(store, row)))
        .collect();
    for (_, switch) in &built {
        add(switch);
    }
    shown.replace(built);
}

/// One wrapper or domain: its switch, and a remove button on a domain the
/// user added (DLG-EXP-02).
fn entry_row(store: &SettingsStore, entry: &expansion::Row) -> adw::SwitchRow {
    let subtitle = if entry.detail.is_empty() {
        String::new()
    } else {
        format!("<tt>{}</tt>", glib::markup_escape_text(&entry.detail))
    };
    let switch = switch_row::switch_row(&glib::markup_escape_text(&entry.label), &subtitle);
    switch.set_active(entry.enabled);
    let id = entry.id.clone();
    switch.connect_active_notify(glib::clone!(
        #[weak]
        store,
        move |switch| {
            let settings = settings(&store);
            if switch.is_active() != settings.is_enabled(&id) {
                store.apply_patch(&expansion::toggle_patch(&settings, &id, switch.is_active()));
            }
        }
    ));
    row::follow_writable(store, &switch, |_| true);
    if entry.removable {
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove this domain")
            .valign(gtk::Align::Center)
            .build();
        remove.add_css_class("flat");
        remove.add_css_class("circular");
        remove.update_property(&[gtk::accessible::Property::Label(&format!(
            "Remove {}",
            entry.label
        ))]);
        let domain = entry.id.clone();
        remove.connect_clicked(glib::clone!(
            #[weak]
            store,
            move |_| store.apply_patch(&expansion::remove_patch(&settings(&store), &domain))
        ));
        row::follow_writable(store, &remove, |_| true);
        switch.add_suffix(&remove);
        // Before the switch, so the switches line up down the list.
        if let Some(suffixes) = remove.parent().and_downcast::<gtk::Box>() {
            suffixes.reorder_child_after(&remove, None::<&gtk::Widget>);
        }
    }
    switch
}

/// The entry that adds a domain: hidden until "+" is pressed.
fn domain_entry() -> adw::EntryRow {
    let entry = adw::EntryRow::builder()
        .title(DOMAIN_TITLE)
        .show_apply_button(true)
        .visible(false)
        .build();
    entry.set_input_purpose(gtk::InputPurpose::Url);
    let cancel = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Cancel")
        .valign(gtk::Align::Center)
        .build();
    cancel.add_css_class("flat");
    cancel.add_css_class("circular");
    cancel.connect_clicked(glib::clone!(
        #[weak]
        entry,
        move |_| {
            entry.set_text("");
            entry.set_visible(false);
            show_error(&entry, "");
        }
    ));
    entry.add_suffix(&cancel);
    entry
}

/// DLG-EXP-02: "+" shows the entry; applying it adds the domain or says
/// why not (in the entry's title, in the error colour).
fn connect_adding(store: &SettingsStore, lists: &Rc<Lists>) {
    let entry = lists.entry.clone();
    lists.short_links.add_button().connect_clicked(glib::clone!(
        #[weak]
        entry,
        move |_| {
            entry.set_visible(true);
            entry.grab_focus();
        }
    ));
    entry.connect_changed(|entry| show_error(entry, ""));
    let weak = Rc::downgrade(lists);
    entry.connect_apply(glib::clone!(
        #[weak]
        store,
        move |entry| {
            let Some(lists) = weak.upgrade() else {
                return;
            };
            let Some(catalogue) = lists.catalogue.borrow().clone() else {
                show_error(entry, "The list is not loaded yet");
                return;
            };
            let settings = settings(&store);
            match expansion::normalise_domain(&entry.text(), &catalogue, &settings) {
                Ok(domain) => {
                    lists.cancel_adding();
                    store.apply_patch(&expansion::add_patch(&settings, &domain));
                }
                Err(error) => show_error(entry, &error.to_string()),
            }
        }
    ));
}

/// Show `message` in the entry's title in the error colour; empty restores
/// the title.
fn show_error(entry: &adw::EntryRow, message: &str) {
    if message.is_empty() {
        entry.set_title(DOMAIN_TITLE);
        entry.remove_css_class("error");
    } else {
        entry.set_title(&glib::markup_escape_text(message));
        entry.add_css_class("error");
    }
}

/// DLG-EXP-04: timeout, maximum redirects, notify on failure.
fn behaviour(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("Behaviour");

    // 0.5 to 5 s in half seconds, shown in seconds.
    let timeout = adw::SpinRow::with_range(0.5, 5.0, TIMEOUT_STEP_S);
    timeout.set_title("Timeout");
    timeout.set_subtitle("Seconds");
    timeout.set_digits(1);
    bind_spin(
        store,
        &timeout,
        |settings| f64::from(settings.timeout_ms) / 1000.0,
        |seconds| {
            // Whole half seconds from 0.5 to 5: exact in an integer.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the spin row keeps the value within 0.5 to 5 seconds"
            )]
            let ms = ((seconds / TIMEOUT_STEP_S).round() * TIMEOUT_STEP_S * 1000.0) as u64;
            ("timeout-ms", Value::from(ms))
        },
    );
    group.add(&timeout);

    let redirects = adw::SpinRow::with_range(1.0, 10.0, 1.0);
    redirects.set_title("Maximum redirects");
    bind_spin(
        store,
        &redirects,
        |settings| f64::from(settings.max_redirects),
        |value| {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the spin row keeps the value within 1 to 10"
            )]
            let count = value.round() as u64;
            ("max-redirects", Value::from(count))
        },
    );
    group.add(&redirects);

    let notify = switch_row::switch_row("Notify when a link cannot be expanded", "");
    switch_row::bind(
        store,
        &notify,
        "advanced.expansion.notify-on-failure",
        false,
    );
    group.add(&notify);
    group
}

/// Show the setting `read` gives and save a change as the key and value
/// `write` makes of it.
fn bind_spin(
    store: &SettingsStore,
    spin: &adw::SpinRow,
    read: impl Fn(&ExpansionSettings) -> f64 + 'static,
    write: impl Fn(f64) -> (&'static str, Value) + 'static,
) {
    let read = Rc::new(read);
    let show = glib::clone!(
        #[weak]
        spin,
        #[strong]
        read,
        move |store: &SettingsStore| {
            let value = read(&settings(store));
            if (spin.value() - value).abs() > f64::EPSILON {
                spin.set_value(value);
            }
        }
    );
    show(store);
    store.connect_changed(show);
    spin.connect_value_notify(glib::clone!(
        #[weak]
        store,
        move |spin| {
            if (spin.value() - read(&settings(&store))).abs() > f64::EPSILON {
                let (key, value) = write(spin.value());
                store.set_value(&format!("advanced.expansion.{key}"), value);
            }
        }
    ));
    row::follow_writable(store, spin, |_| true);
}

/// `advanced.expansion` now.
fn settings(store: &SettingsStore) -> ExpansionSettings {
    store.with_snapshot(|s| s.typed_config().advanced.expansion)
}
