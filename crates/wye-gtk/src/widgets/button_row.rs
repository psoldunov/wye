//! BLK-05 Button row: an `AdwActionRow` with a push button as its trailing
//! control ("Choose…", "Configure…", "Edit Script…", "Make Default"). A
//! button can sit next to a switch in the same row ("Configure…" beside an
//! on/off switch).
//!
//! API:
//! - [`ButtonRow::new`]`(title, subtitle, label)`: the row and its button.
//! - [`ButtonRow::set_suggested`]: the row's suggested action, drawn in the
//!   accent colour (GEN-05's "Make Default").
//! - [`ButtonRow::add_switch`]: a switch after the button; bind it like a
//!   switch row's value with [`ButtonRow::bind_switch`].
//! - [`ButtonRow::activate_with_row`]: clicking anywhere on the row presses
//!   the button.
//! - `row.button().connect_clicked(…)` for what the button does.
//! - [`name_button`]`(button, name)`: a text button's own accessible name.

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::row;
use crate::settings::store::SettingsStore;

/// A row with a button and optionally a switch.
#[derive(Debug, Clone)]
pub struct ButtonRow {
    row: adw::ActionRow,
    button: gtk::Button,
    switch: Option<gtk::Switch>,
}

impl ButtonRow {
    /// A row titled `title` (subtitle unless empty) with a button `label`.
    /// Screen readers hear the button with its row ("Choose: Shown
    /// browsers"), so two "Configure…" buttons are told apart.
    #[must_use]
    pub fn new(title: &str, subtitle: &str, label: &str) -> Self {
        let row = row::action_row(title, subtitle);
        let button = gtk::Button::builder()
            .label(label)
            .valign(gtk::Align::Center)
            .build();
        name_button(&button, &accessible_name(label, title));
        row.add_suffix(&button);
        Self {
            row,
            button,
            switch: None,
        }
    }

    /// The row, to add to a group.
    #[must_use]
    pub fn row(&self) -> &adw::ActionRow {
        &self.row
    }

    /// The button.
    #[must_use]
    pub fn button(&self) -> &gtk::Button {
        &self.button
    }

    /// The switch [`Self::add_switch`] added.
    #[must_use]
    pub fn switch(&self) -> Option<&gtk::Switch> {
        self.switch.as_ref()
    }

    /// Make the button the row's suggested action (accent colour) or a plain
    /// button.
    pub fn set_suggested(&self, suggested: bool) {
        if suggested {
            self.button.add_css_class("suggested-action");
        } else {
            self.button.remove_css_class("suggested-action");
        }
    }

    /// Let a click anywhere on the row press the button. The row then
    /// labels the button with its title; the button keeps its own name.
    pub fn activate_with_row(&self) {
        self.row.set_activatable_widget(Some(&self.button));
        let label = self.button.label().unwrap_or_default();
        name_button(&self.button, &accessible_name(&label, &self.row.title()));
    }

    /// Change the button's label.
    pub fn set_label(&self, label: &str) {
        self.button.set_label(label);
        name_button(&self.button, &accessible_name(label, &self.row.title()));
    }

    /// Put a switch after the button ("Configure…" next to on/off).
    #[must_use]
    pub fn add_switch(mut self) -> Self {
        let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        self.row.add_suffix(&switch);
        self.row.set_activatable_widget(Some(&switch));
        self.switch = Some(switch);
        self
    }

    /// Tie the switch to the boolean at `path` (see
    /// [`super::switch_row::bind`]). The button follows the switch: it is
    /// sensitive only while the switch is on, when `button_needs_on`.
    pub fn bind_switch(
        &self,
        store: &SettingsStore,
        path: &'static str,
        default: bool,
        button_needs_on: bool,
    ) {
        let Some(switch) = self.switch.clone() else {
            tracing::warn!(%path, "bind_switch on a button row without a switch");
            return;
        };
        let button = self.button.clone();
        let show = glib::clone!(
            #[weak]
            switch,
            #[weak]
            button,
            move |store: &SettingsStore| {
                let on = store.bool_value(path, default);
                if switch.is_active() != on {
                    switch.set_active(on);
                }
                button.set_sensitive(!button_needs_on || on);
            }
        );
        show(store);
        store.connect_changed_while(&switch, show);
        switch.connect_active_notify(glib::clone!(
            #[weak]
            store,
            move |switch| {
                if switch.is_active() != store.bool_value(path, default) {
                    store.set_value(path, Value::Bool(switch.is_active()));
                }
            }
        ));
        row::follow_writable(store, &switch, |_| true);
    }
}

/// Name `button` `name` for screen readers. A button with text is
/// labelled by its label, which wins over a name of its own, so that
/// relation goes (`set_label` brings it back: name the button again after).
pub fn name_button(button: &gtk::Button, name: &str) {
    button.reset_relation(gtk::AccessibleRelation::LabelledBy);
    button.update_property(&[gtk::accessible::Property::Label(name)]);
    // An `AdwButtonContent` child labels its button again once on screen.
    button.connect_realize(|button| {
        button.reset_relation(gtk::AccessibleRelation::LabelledBy);
    });
}

/// "Choose: Shown browsers": the button's words without their ellipsis,
/// then its row's title; the words alone in an untitled row.
fn accessible_name(label: &str, title: &str) -> String {
    let words = label.trim_end_matches('…');
    if title.is_empty() {
        label.to_owned()
    } else {
        format!("{words}: {title}")
    }
}

#[cfg(test)]
mod tests {
    use super::accessible_name;

    #[test]
    fn a_button_is_named_after_its_row() {
        assert_eq!(
            accessible_name("Choose…", "Shown browsers"),
            "Choose: Shown browsers"
        );
        assert_eq!(accessible_name("Make Default", ""), "Make Default");
    }
}
