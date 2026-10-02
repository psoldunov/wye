//! BLK-07 Text entry row: an `AdwEntryRow`, the row whose title moves above
//! the text while it is edited ("Name"), with an apply button: the form
//! that owns the row takes the text on apply (SET-06 saves whole values).
//!
//! API:
//! - [`EntryRow::new`]`(title)`: the row.
//! - [`EntryRow::set_validator`]: a check run on every edit; a message
//!   marks the row as an error (`.error`) with the message as its tooltip
//!   and keeps it from being saved.
//! - [`focus_when_shown`]`(dialog, row, position)`: give an `AdwEntryRow`
//!   in a dialog the focus once the dialog is on screen, the caret at
//!   `position` (-1: the end) and nothing selected.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

/// Checks a text; `Some(message)` when it is not valid.
type Validator = Rc<dyn Fn(&str) -> Option<String>>;

/// An entry row.
#[derive(Clone)]
pub struct EntryRow {
    row: adw::EntryRow,
    validator: Rc<RefCell<Option<Validator>>>,
}

impl std::fmt::Debug for EntryRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EntryRow")
            .field("row", &self.row)
            .finish_non_exhaustive()
    }
}

impl EntryRow {
    /// An entry row titled `title`, with an apply button.
    #[must_use]
    pub fn new(title: &str) -> Self {
        let row = adw::EntryRow::builder()
            .title(title)
            .show_apply_button(true)
            .build();
        let this = Self {
            row,
            validator: Rc::default(),
        };
        let validator = Rc::clone(&this.validator);
        this.row.connect_changed(move |row| {
            let message = validator
                .borrow()
                .as_ref()
                .and_then(|check| check(&row.text()));
            mark(row, message.as_deref());
        });
        this
    }

    /// The row, to add to a group.
    #[must_use]
    pub fn row(&self) -> &adw::EntryRow {
        &self.row
    }

    /// Check every edit with `check`; see the module docs.
    pub fn set_validator(&self, check: impl Fn(&str) -> Option<String> + 'static) {
        self.validator.replace(Some(Rc::new(check)));
    }

    /// Whether the text passes the validator.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validator
            .borrow()
            .as_ref()
            .is_none_or(|check| check(&self.row.text()).is_none())
    }
}

fn mark(row: &adw::EntryRow, message: Option<&str>) {
    if message.is_some() {
        row.add_css_class("error");
    } else {
        row.remove_css_class("error");
    }
    row.set_tooltip_text(message);
}

/// Focus `row` once `dialog` is on screen, the caret at `position` (-1 for
/// the end), without selecting its text, as KDE places the caret. Focusing
/// an entry selects all of it, and the dialog grabs its focus again as it
/// maps and as its window becomes active (on Wayland, after the compositor
/// configured it), so every focus the row gets is placed that way until the
/// user first presses a key or clicks in it; from then on the row behaves
/// as any entry does.
pub fn focus_when_shown(dialog: &adw::Dialog, row: &adw::EntryRow, position: i32) {
    let Some(text) = row.delegate().and_downcast::<gtk::Text>() else {
        dialog.set_focus(Some(row));
        return;
    };
    let placing = Rc::new(std::cell::Cell::new(true));
    let place = glib::clone!(
        #[weak]
        text,
        move || {
            text.select_region(position, position);
            text.set_position(position);
        }
    );
    let place = Rc::new(place);
    text.connect_has_focus_notify(glib::clone!(
        #[strong]
        placing,
        #[strong]
        place,
        move |text| {
            if placing.get() && text.has_focus() {
                // After the grab's own select-all.
                let place = Rc::clone(&place);
                glib::idle_add_local_once(move || place());
            }
        }
    ));
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        placing,
        move |_, _, _, _| {
            placing.set(false);
            glib::Propagation::Proceed
        }
    ));
    text.add_controller(keys);
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed(move |_, _, _, _| placing.set(false));
    text.add_controller(click);
    dialog.set_focus(Some(&text));
    place();
}
