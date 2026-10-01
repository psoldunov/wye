//! A popup row of fixed choices: an `AdwComboRow` whose items are human
//! labels for configuration values (GEN-02 "Tray icon": Primary Browser or
//! Wye). The stored value never shows: `primary-browser` reads "Primary
//! Browser".
//!
//! API:
//! - [`ChoiceRow::new`]`(title, subtitle, choices)`: `choices` are
//!   `(value, label)` pairs in menu order.
//! - [`ChoiceRow::bind`]`(store, path, default)`: show the value at `path`,
//!   save a choice (SET-06), follow `writable`. A value the row does not
//!   know (a newer version wrote it) shows as `default` and stays in the
//!   file until the user picks a value.
//! - [`ChoiceRow::connect_chosen`]: run code on a choice instead of binding.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::row;
use crate::settings::store::SettingsStore;

/// A combo row of fixed `(value, label)` choices.
#[derive(Debug, Clone)]
pub struct ChoiceRow {
    row: adw::ComboRow,
    values: Rc<[&'static str]>,
    /// Set while code selects a value, so it is not taken as the user's.
    showing: Rc<Cell<bool>>,
}

impl ChoiceRow {
    /// A row titled `title` offering `choices`.
    #[must_use]
    pub fn new(title: &str, subtitle: &str, choices: &[(&'static str, &str)]) -> Self {
        let labels: Vec<&str> = choices.iter().map(|(_, label)| *label).collect();
        let row = row::titled(
            adw::ComboRow::builder()
                .model(&gtk::StringList::new(&labels))
                .build(),
            title,
            subtitle,
        );
        Self {
            row,
            values: choices.iter().map(|(value, _)| *value).collect(),
            showing: Rc::default(),
        }
    }

    /// The row, to add to a group.
    #[must_use]
    pub fn row(&self) -> &adw::ComboRow {
        &self.row
    }

    /// Show `value`; an unknown value selects nothing.
    pub fn set_value(&self, value: &str) {
        select(&self.row, &self.values, &self.showing, value);
    }

    /// Run `chosen` with the value the user picks; a value code shows is
    /// not one.
    pub fn connect_chosen(&self, chosen: impl Fn(&'static str) + 'static) {
        let values = Rc::clone(&self.values);
        let showing = Rc::clone(&self.showing);
        self.row.connect_selected_notify(move |row| {
            if showing.get() {
                return;
            }
            let value = usize::try_from(row.selected())
                .ok()
                .and_then(|index| values.get(index).copied());
            if let Some(value) = value {
                chosen(value);
            }
        });
    }

    /// Tie the row to the string at `path`. See the module docs.
    pub fn bind(&self, store: &SettingsStore, path: &'static str, default: &'static str) {
        let values = Rc::clone(&self.values);
        let showing = Rc::clone(&self.showing);
        let show = glib::clone!(
            #[weak(rename_to = row)]
            self.row,
            move |store: &SettingsStore| {
                let stored = store.string_value(path, default);
                let shown = if values.contains(&stored.as_str()) {
                    stored.as_str()
                } else {
                    default
                };
                select(&row, &values, &showing, shown);
            }
        );
        show(store);
        store.connect_changed_while(&self.row, show);
        self.connect_chosen(glib::clone!(
            #[weak]
            store,
            move |value| {
                if store.string_value(path, default) != value {
                    store.set_value(path, Value::String(value.to_owned()));
                }
            }
        ));
        row::follow_writable(store, &self.row, |_| true);
    }
}

/// Select `value` among `values` in `row`; nothing for an unknown value.
/// `showing` is set meanwhile: the selection is the store's, not a choice.
fn select(row: &adw::ComboRow, values: &[&'static str], showing: &Cell<bool>, value: &str) {
    let index = values
        .iter()
        .position(|known| *known == value)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(gtk::INVALID_LIST_POSITION);
    if row.selected() != index {
        showing.set(true);
        row.set_selected(index);
        showing.set(false);
    }
}
