//! BLK-06 Inline radio row: an `AdwActionRow` whose trailing control is an
//! `AdwToggleGroup`, a horizontal set of mutually exclusive choices
//! ("Small", "Medium", "Large").
//!
//! API:
//! - [`RadioRow::new`]`(title, subtitle, choices)`: `(value, label)` pairs.
//! - [`RadioRow::bind`]`(store, path, default)`: show the string at `path`,
//!   save a choice (SET-06), follow `writable`.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::row;
use crate::settings::store::SettingsStore;

/// A row with a toggle group.
#[derive(Debug, Clone)]
pub struct RadioRow {
    row: adw::ActionRow,
    toggles: adw::ToggleGroup,
    /// Set while code shows the stored value, so it is not taken as the
    /// user's choice.
    showing: Rc<Cell<bool>>,
}

impl RadioRow {
    /// A row titled `title` with one toggle per `(value, label)`.
    #[must_use]
    pub fn new(title: &str, subtitle: &str, choices: &[(&str, &str)]) -> Self {
        let row = row::action_row(title, subtitle);
        let toggles = adw::ToggleGroup::builder()
            .valign(gtk::Align::Center)
            .build();
        for (value, label) in choices {
            toggles.add(adw::Toggle::builder().name(*value).label(*label).build());
        }
        row.add_suffix(&toggles);
        Self {
            row,
            toggles,
            showing: Rc::default(),
        }
    }

    /// The row, to add to a group.
    #[must_use]
    pub fn row(&self) -> &adw::ActionRow {
        &self.row
    }

    /// The toggle group, for the kit's GTK test to choose as a user does.
    #[cfg(test)]
    pub(crate) fn toggles(&self) -> &adw::ToggleGroup {
        &self.toggles
    }

    /// Tie the group to the string at `path`. See the module docs.
    pub fn bind(&self, store: &SettingsStore, path: &'static str, default: &'static str) {
        let showing = Rc::clone(&self.showing);
        let show = glib::clone!(
            #[weak(rename_to = toggles)]
            self.toggles,
            move |store: &SettingsStore| {
                let value = store.string_value(path, default);
                let known = toggles.toggle_by_name(&value).is_some();
                let shown = if known { value.as_str() } else { default };
                if toggles.active_name().as_deref() != Some(shown) {
                    showing.set(true);
                    toggles.set_active_name(Some(shown));
                    showing.set(false);
                }
            }
        );
        show(store);
        store.connect_changed_while(&self.toggles, show);
        let showing = Rc::clone(&self.showing);
        self.toggles.connect_active_name_notify(glib::clone!(
            #[weak]
            store,
            move |toggles| {
                // A value the store pushed (an unknown one shown as the
                // default) is never written back.
                if showing.get() {
                    return;
                }
                let Some(name) = toggles.active_name() else {
                    return;
                };
                if store.string_value(path, default) != name.as_str() {
                    store.set_value(path, Value::String(name.to_string()));
                }
            }
        ));
        row::follow_writable(store, &self.toggles, |_| true);
    }
}
