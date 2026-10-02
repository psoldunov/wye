//! BLK-18 Modifier chooser: four linked toggle buttons, **Shift**, **Ctrl**,
//! **Alt**, **Super**, as a row's trailing control. The pressed set is the
//! binding (KEY-01); none pressed is off.
//!
//! API:
//! - [`ModifierChooser::new`]`()`: the buttons.
//! - [`ModifierChooser::add_row`]`(title, subtitle)`: a row with the chooser
//!   as its control.
//! - [`ModifierChooser::set_pressed`]`(names)`: show a binding.
//! - [`ModifierChooser::connect_changed`]`(|names|)`: the user pressed or
//!   released one; `names` in Shift, Ctrl, Alt, Super order.
//! - [`ModifierChooser::bind`]`(store, path)`: show the set at `path`
//!   (`browsers.alternative-key`), save a change (SET-06), follow
//!   `writable`. A page that checks clashes first (KEY-21) uses
//!   `connect_changed` instead.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::row;
use crate::settings::store::SettingsStore;

/// The buttons, in order, as the configuration names them.
pub const MODIFIERS: [&str; 4] = ["Shift", "Ctrl", "Alt", "Super"];

type Changed = dyn Fn(&[String]);

/// Four linked toggle buttons. Clones share them.
#[derive(Clone)]
pub struct ModifierChooser {
    widget: gtk::Box,
    buttons: Vec<gtk::ToggleButton>,
    /// Set while code changes the buttons, so no change is reported.
    showing: Rc<Cell<bool>>,
    changed: Rc<RefCell<Option<Rc<Changed>>>>,
}

impl std::fmt::Debug for ModifierChooser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModifierChooser")
            .field("pressed", &self.pressed())
            .finish_non_exhaustive()
    }
}

impl Default for ModifierChooser {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifierChooser {
    /// Four released buttons.
    #[must_use]
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .valign(gtk::Align::Center)
            .css_classes(["linked"])
            .build();
        let buttons: Vec<gtk::ToggleButton> = MODIFIERS
            .iter()
            .map(|name| gtk::ToggleButton::builder().label(*name).build())
            .collect();
        for button in &buttons {
            widget.append(button);
        }
        let this = Self {
            widget,
            buttons,
            showing: Rc::default(),
            changed: Rc::default(),
        };
        for button in &this.buttons {
            let showing = Rc::clone(&this.showing);
            let changed = Rc::clone(&this.changed);
            let buttons = this
                .buttons
                .iter()
                .map(glib::object::ObjectExt::downgrade)
                .collect::<Vec<_>>();
            button.connect_toggled(move |_| {
                if showing.get() {
                    return;
                }
                let pressed: Vec<String> = buttons
                    .iter()
                    .zip(MODIFIERS)
                    .filter(|(button, _)| button.upgrade().is_some_and(|b| b.is_active()))
                    .map(|(_, name)| name.to_owned())
                    .collect();
                let callback = changed.borrow().clone();
                if let Some(callback) = callback {
                    callback(&pressed);
                }
            });
        }
        this
    }

    /// The buttons' box.
    #[must_use]
    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    /// A row titled `title` with this chooser as its control.
    #[must_use]
    pub fn add_row(&self, title: &str, subtitle: &str) -> adw::ActionRow {
        let row = row::action_row(title, subtitle);
        row.add_suffix(&self.widget);
        row
    }

    /// The pressed buttons' names.
    #[must_use]
    pub fn pressed(&self) -> Vec<String> {
        self.buttons
            .iter()
            .zip(MODIFIERS)
            .filter(|(button, _)| button.is_active())
            .map(|(_, name)| name.to_owned())
            .collect()
    }

    /// Press exactly the buttons named in `names` (case-insensitive).
    pub fn set_pressed(&self, names: &[String]) {
        press(&self.buttons, &self.showing, names);
    }

    /// Run `changed` with the pressed names after the user changes them.
    pub fn connect_changed(&self, changed: impl Fn(&[String]) + 'static) {
        self.changed.replace(Some(Rc::new(changed)));
    }

    /// Tie the chooser to the modifier set at `path`. See the module docs.
    pub fn bind(&self, store: &SettingsStore, path: &'static str) {
        // The buttons weakly: the handler goes with the chooser.
        let buttons: Vec<_> = self
            .buttons
            .iter()
            .map(glib::object::ObjectExt::downgrade)
            .collect();
        let showing = Rc::clone(&self.showing);
        let show = move |store: &SettingsStore| {
            let alive: Option<Vec<gtk::ToggleButton>> =
                buttons.iter().map(glib::WeakRef::upgrade).collect();
            if let Some(alive) = alive {
                press(&alive, &showing, &stored(store, path));
            }
        };
        show(store);
        store.connect_changed_while(&self.widget, show);
        self.connect_changed(glib::clone!(
            #[weak]
            store,
            move |names| {
                if names != stored(&store, path).as_slice() {
                    store.set_modifiers(path, names);
                }
            }
        ));
        row::follow_writable(store, &self.widget, |_| true);
    }
}

/// Press exactly the `buttons` named in `names` (case-insensitive), with
/// `showing` set so the change is not taken as the user's.
fn press(buttons: &[gtk::ToggleButton], showing: &Cell<bool>, names: &[String]) {
    showing.set(true);
    for (button, name) in buttons.iter().zip(MODIFIERS) {
        let on = names
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name));
        if button.is_active() != on {
            button.set_active(on);
        }
    }
    showing.set(false);
}

/// The names stored at `path`, in Shift, Ctrl, Alt, Super order.
fn stored(store: &SettingsStore, path: &str) -> Vec<String> {
    let names: Vec<String> = store
        .value(path)
        .and_then(|value| match value {
            Value::Array(items) => Some(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    MODIFIERS
        .iter()
        .filter(|name| names.iter().any(|stored| stored.eq_ignore_ascii_case(name)))
        .map(|name| (*name).to_owned())
        .collect()
}
