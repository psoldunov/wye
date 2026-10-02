//! The Keyboard Shortcuts group of the Advanced page (ADV-05 to ADV-07,
//! BLK-16, KEY-40, KEY-41): one row per global action, whose control follows
//! the session's mechanism:
//!
//! - portal: the binding the portal reports ("None" when unset) and
//!   **Change…**, which opens the desktop's own shortcut dialog; the portal
//!   owns the binding, so that dialog is also where it is removed;
//! - X11: the recorder ("None" when unset) and a clear button;
//! - none: the command to bind in the compositor's configuration, with
//!   **Copy** and a help button (KEY-41).
//!
//! KDE counterpart: crates/wye-ui/qml/components/WyeGlobalShortcutRow.qml.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use super::shortcuts::{self, Row, View};
use crate::settings::store::SettingsStore;
use crate::widgets::shortcut::{self as recorder, Recorded, ShortcutRecorder};
use crate::widgets::{button_row, group, help, row, toast};

/// The group and the rows it shows now.
#[derive(Debug, Clone)]
pub struct ShortcutRows {
    group: adw::PreferencesGroup,
    rows: Rc<RefCell<Vec<adw::ActionRow>>>,
    shown: Rc<RefCell<Option<View>>>,
}

impl ShortcutRows {
    /// The empty group; [`Self::load`] fills it.
    #[must_use]
    pub fn new(store: &SettingsStore) -> Self {
        let group = group::group("Keyboard Shortcuts");
        let this = Self {
            group,
            rows: Rc::default(),
            shown: Rc::default(),
        };
        // Until the service answers: what the configuration holds.
        let fallback = store.with_snapshot(|s| shortcuts::fallback(&s.config));
        this.show(store, &fallback);
        this
    }

    /// The group, to add to the page.
    #[must_use]
    pub fn group(&self) -> &adw::PreferencesGroup {
        &self.group
    }

    /// Read the shortcuts again (`GetShortcuts`, or the fixture's) and show
    /// them.
    pub fn load(&self, store: &SettingsStore) {
        let this = self.clone();
        let shown = store.downgrade();
        store.shortcuts(move |wire| {
            let Some(store) = shown.upgrade() else {
                return;
            };
            let view = store.with_snapshot(|s| {
                wire.map_or_else(
                    || shortcuts::fallback(&s.config),
                    |wire| shortcuts::from_wire(&wire, &s.config),
                )
            });
            this.show(&store, &view);
        });
    }

    /// Show `view`; the rows are built again only when it changed.
    fn show(&self, store: &SettingsStore, view: &View) {
        if self.shown.borrow().as_ref() == Some(view) {
            return;
        }
        for old in self.rows.borrow_mut().drain(..) {
            self.group.remove(&old);
        }
        let built: Vec<adw::ActionRow> = view
            .rows
            .iter()
            .map(|shortcut| self.build(store, view, shortcut))
            .collect();
        // Rows go into the group's list; the note under it is not a row and
        // stays below.
        for row in &built {
            self.group.add(row);
        }
        self.rows.replace(built);
        self.shown.replace(Some(view.clone()));
    }

    fn build(&self, store: &SettingsStore, view: &View, shortcut: &Row) -> adw::ActionRow {
        let row = row::action_row(&shortcut.title, "");
        if !view.recordable {
            unbindable(store, &row, shortcut);
        } else if view.configurable {
            portal(store, &row, shortcut);
        } else {
            self.recordable(store, &row, shortcut);
        }
        row
    }

    /// X11: the recorder, and a clear button once a shortcut is set.
    fn recordable(&self, store: &SettingsStore, row: &adw::ActionRow, shortcut: &Row) {
        let bound = !shortcut.binding.is_empty();
        let recording = ShortcutRecorder::new("None");
        recording.set_binding(bound.then_some(shortcut.binding.as_str()));
        recording
            .widget()
            .update_property(&[gtk::accessible::Property::Label(&if bound {
                format!("Shortcut: {}", shortcut.label)
            } else {
                "No shortcut".to_owned()
            })]);
        let clear = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Clear shortcut")
            .valign(gtk::Align::Center)
            .sensitive(bound)
            .opacity(if bound { 1.0 } else { 0.0 })
            .build();
        clear.add_css_class("flat");
        clear.add_css_class("circular");
        clear.update_property(&[gtk::accessible::Property::Label(&format!(
            "Clear shortcut: {}",
            shortcut.title
        ))]);
        row.add_suffix(recording.widget());
        row.add_suffix(&clear);

        let action = shortcut.action.clone();
        let (this, owner) = (self.clone(), store.downgrade());
        recording.connect_recorded(move |outcome| {
            let Some(store) = owner.upgrade() else {
                return;
            };
            let binding = match outcome {
                Recorded::Binding { stored, .. } => stored.clone(),
                Recorded::Clear => String::new(),
                Recorded::Cancel => return,
            };
            this.save(&store, &action, &binding);
        });
        let action = shortcut.action.clone();
        let (this, owner) = (self.clone(), store.downgrade());
        clear.connect_clicked(move |_| {
            if let Some(store) = owner.upgrade() {
                this.save(&store, &action, "");
            }
        });
    }

    fn save(&self, store: &SettingsStore, action: &str, binding: &str) {
        let (this, owner) = (self.clone(), store.downgrade());
        store.set_shortcut(action, binding, move || {
            if let Some(store) = owner.upgrade() {
                this.load(&store);
            }
        });
    }
}

/// KEY-40: the binding the portal reports and Change… for its dialog.
fn portal(store: &SettingsStore, row: &adw::ActionRow, shortcut: &Row) {
    let shown: gtk::Widget = if let Some(accelerator) = portal_accelerator(&shortcut.binding) {
        adw::ShortcutLabel::new(&accelerator).upcast()
    } else {
        let text = if shortcut.binding.is_empty() {
            "None"
        } else {
            shortcut.label.as_str()
        };
        let label = gtk::Label::new(Some(text));
        label.add_css_class("dimmed");
        label.upcast()
    };
    shown.set_valign(gtk::Align::Center);
    shown.update_property(&[gtk::accessible::Property::Label(
        &if shortcut.binding.is_empty() {
            "No shortcut".to_owned()
        } else {
            format!("Shortcut: {}", shortcut.label)
        },
    )]);
    let change = gtk::Button::builder()
        .label("Change…")
        .tooltip_text("Change or remove this shortcut in the desktop's shortcut settings")
        .valign(gtk::Align::Center)
        .build();
    change.connect_clicked(glib::clone!(
        #[weak]
        store,
        move |_| store.call(|proxy| async move { proxy.configure_shortcuts().await })
    ));
    row.add_suffix(&shown);
    row.add_suffix(&change);
}

/// KEY-41: no mechanism; the command to bind by hand, and Copy.
fn unbindable(store: &SettingsStore, row: &adw::ActionRow, shortcut: &Row) {
    let command = glib::markup_escape_text(&shortcut.command);
    row.set_subtitle(&format!(
        "Bind <tt>{command}</tt> in your compositor's configuration."
    ));
    help::add_help(store, row, "global-shortcuts");
    let content = adw::ButtonContent::builder()
        .icon_name("edit-copy-symbolic")
        .label("Copy")
        .build();
    let copy = gtk::Button::builder()
        .child(&content)
        .tooltip_text("Copy the command")
        .valign(gtk::Align::Center)
        .build();
    // One per shortcut: named after its row, so they are told apart.
    button_row::name_button(&copy, &format!("Copy command: {}", row.title()));
    let text = shortcut.command.clone();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(&text);
        toast::show(button, adw::Toast::new(&format!("Copied “{text}”")));
    });
    row.add_suffix(&copy);
}

/// A trigger as a portal reports it ("CTRL+ALT+O", "<Control><Alt>o") or as
/// Wye stores it ("Ctrl+Alt+o"), as a GTK accelerator for
/// `AdwShortcutLabel`; `None` when GTK cannot read it.
fn portal_accelerator(trigger: &str) -> Option<String> {
    if trigger.is_empty() {
        return None;
    }
    if trigger.starts_with('<') {
        return gtk::accelerator_parse(trigger).map(|_| trigger.to_owned());
    }
    let mut parts: Vec<&str> = trigger.split('+').collect();
    let key = parts.pop()?;
    let modifiers: Option<Vec<&str>> = parts
        .iter()
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => Some("Ctrl"),
            "alt" => Some("Alt"),
            "shift" => Some("Shift"),
            "super" | "logo" | "meta" => Some("Super"),
            _ => None,
        })
        .collect();
    let key = if key.chars().count() == 1 {
        key.to_lowercase()
    } else {
        key.to_owned()
    };
    let stored = modifiers?
        .into_iter()
        .chain(std::iter::once(key.as_str()))
        .collect::<Vec<_>>()
        .join("+");
    recorder::accelerator(&stored)
        .filter(|accelerator| gtk::accelerator_parse(accelerator).is_some())
}
