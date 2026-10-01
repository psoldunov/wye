//! BLK-16 Shortcut recorder, in the style of GNOME Settings' keyboard
//! shortcuts: a button showing the binding as key caps (`AdwShortcutLabel`),
//! or "Record Shortcut" (not dimmed) until one is set. Clicking it opens a
//! small dialog that waits for a key combination: Escape cancels, Backspace
//! clears, a modifier on its own waits for the key it modifies, anything
//! else is the binding, stored with XKB names (KEY-02, KEY-03), as
//! `wye_core::keybinding` reads them.
//!
//! Where an action takes several bindings, [`ShortcutChips`] shows each as a
//! removable chip followed by "+" (KEY-02).
//!
//! The global shortcut row (ADV-05 to ADV-07) picks its control by session:
//! the portal's own dialog ("Change…", a [`super::button_row::ButtonRow`]),
//! this recorder on X11, or the command and **Copy**; that choice belongs to
//! the Advanced page.
//!
//! API:
//! - [`record`]`(parent, title, single, done)`: the recording dialog alone.
//! - [`ShortcutRecorder::new`]`(empty_text)`, [`ShortcutRecorder::set_binding`],
//!   [`ShortcutRecorder::connect_recorded`]; [`ShortcutRecorder::widget`] is
//!   the button, for a row's suffix.
//! - [`ShortcutChips::new`]`()`, [`ShortcutChips::set_bindings`],
//!   [`ShortcutChips::connect_added`], [`ShortcutChips::connect_removed`].

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};
use wye_core::keybinding::{KeyBinding, parse_bindings};
use wye_core::{Modifier, Modifiers};

/// What a key press did to a recorder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    /// Escape: stop recording, change nothing.
    Cancel,
    /// Backspace: clear the binding.
    Clear,
    /// The binding, as stored ("Ctrl+Shift+o") and as shown ("Ctrl+Shift+O").
    Binding { stored: String, label: String },
}

/// The keys that only modify another.
const MODIFIER_KEYS: [gdk::Key; 12] = [
    gdk::Key::Shift_L,
    gdk::Key::Shift_R,
    gdk::Key::Control_L,
    gdk::Key::Control_R,
    gdk::Key::Alt_L,
    gdk::Key::Alt_R,
    gdk::Key::Super_L,
    gdk::Key::Super_R,
    gdk::Key::Meta_L,
    gdk::Key::Meta_R,
    gdk::Key::ISO_Level3_Shift,
    gdk::Key::Caps_Lock,
];

/// The held modifiers of `state`, as Wye names them.
fn modifiers(state: gdk::ModifierType) -> Modifiers {
    let pairs = [
        (gdk::ModifierType::CONTROL_MASK, Modifier::Ctrl),
        (gdk::ModifierType::ALT_MASK, Modifier::Alt),
        (gdk::ModifierType::SHIFT_MASK, Modifier::Shift),
        (gdk::ModifierType::SUPER_MASK, Modifier::Super),
    ];
    let held: Vec<Modifier> = pairs
        .iter()
        .filter(|(mask, _)| state.contains(*mask))
        .map(|(_, modifier)| *modifier)
        .collect();
    Modifiers::from_slice(&held)
}

/// The outcome of one key press, `None` while only a modifier is down. With
/// `single` (a hotkey, KEY-12) the modifiers are ignored.
#[must_use]
pub fn interpret(key: gdk::Key, state: gdk::ModifierType, single: bool) -> Option<Recorded> {
    if MODIFIER_KEYS.contains(&key) {
        return None;
    }
    let held = modifiers(state);
    if held.is_empty() {
        if key == gdk::Key::Escape {
            return Some(Recorded::Cancel);
        }
        if key == gdk::Key::BackSpace {
            return Some(Recorded::Clear);
        }
    }
    // The key without Shift's effect: Shift+o is stored as "Shift+o".
    let name = key.to_lower().name()?;
    let held = if single { Modifiers::NONE } else { held };
    let binding = KeyBinding::new(held, &name).ok()?;
    Some(Recorded::Binding {
        stored: binding.stored(),
        label: binding.label(),
    })
}

/// A stored binding as a GTK accelerator (`<Control><Shift>o`), for
/// `AdwShortcutLabel`; `None` for a modifier GTK has no name for. Whether
/// GTK knows the key is checked where it is shown.
#[must_use]
pub fn accelerator(stored: &str) -> Option<String> {
    let mut parts: Vec<&str> = stored.split('+').collect();
    let key = parts.pop().filter(|key| !key.is_empty())?;
    let mut accel = String::new();
    for part in parts {
        let gtk_name = match part {
            "Ctrl" => "<Control>",
            "Alt" => "<Alt>",
            "Shift" => "<Shift>",
            "Super" => "<Super>",
            _ => return None,
        };
        accel.push_str(gtk_name);
    }
    accel.push_str(key);
    Some(accel)
}

/// How a stored binding reads ("Ctrl+Shift+O"); as stored when it does not
/// parse.
#[must_use]
pub fn label(stored: &str) -> String {
    let (parsed, _) = parse_bindings(&[stored.to_owned()]);
    parsed
        .first()
        .map_or_else(|| stored.to_owned(), KeyBinding::label)
}

/// Open the recording dialog over `parent`'s window; `done` gets the result
/// once (Escape and closing the dialog are [`Recorded::Cancel`]).
pub fn record(
    parent: &impl IsA<gtk::Widget>,
    title: &str,
    single: bool,
    done: impl Fn(Recorded) + 'static,
) {
    let dialog = recording_dialog(title);
    let done: Rc<dyn Fn(Recorded)> = Rc::new(done);
    let answered = Rc::new(std::cell::Cell::new(false));
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        done,
        #[strong]
        answered,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, state| {
            let Some(outcome) = interpret(key, state, single) else {
                return glib::Propagation::Stop;
            };
            answered.set(true);
            done(outcome);
            dialog.close();
            glib::Propagation::Stop
        }
    ));
    dialog.add_controller(keys);
    dialog.connect_closed(move |_| {
        if !answered.replace(true) {
            done(Recorded::Cancel);
        }
    });
    dialog.present(Some(parent));
}

/// The "Set Shortcut" dialog: `title`, a keyboard, and how to cancel or
/// clear (BLK-16).
fn recording_dialog(title: &str) -> adw::Dialog {
    let heading = gtk::Label::builder()
        .label(title)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["title-4"])
        .build();
    let picture = gtk::Image::builder()
        .icon_name("input-keyboard-symbolic")
        .pixel_size(96)
        .margin_top(12)
        .margin_bottom(12)
        .css_classes(["wye-shortcut-keyboard"])
        .build();
    let hint = gtk::Label::builder()
        .label("Press Esc to cancel or Backspace to disable the keyboard shortcut.")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dimmed"])
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    content.append(&heading);
    content.append(&picture);
    content.append(&hint);
    let header = adw::HeaderBar::builder().show_title(false).build();
    let view = adw::ToolbarView::builder().content(&content).build();
    view.add_top_bar(&header);
    let dialog = adw::Dialog::builder()
        .title("Set Shortcut")
        .child(&view)
        .content_width(400)
        .build();
    dialog.add_css_class("wye-shortcut-dialog");
    dialog
}

type RecordedCallback = dyn Fn(&Recorded);

/// A button that records one binding.
#[derive(Clone)]
pub struct ShortcutRecorder {
    button: gtk::Button,
    stack: gtk::Stack,
    shortcut: adw::ShortcutLabel,
    plain: gtk::Label,
    recorded: Rc<RefCell<Option<Rc<RecordedCallback>>>>,
}

impl std::fmt::Debug for ShortcutRecorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShortcutRecorder")
            .field("button", &self.button)
            .finish_non_exhaustive()
    }
}

impl ShortcutRecorder {
    /// A recorder reading `empty_text` ("Record Shortcut") while unset.
    #[must_use]
    pub fn new(empty_text: &str) -> Self {
        let shortcut = adw::ShortcutLabel::new("");
        let plain = gtk::Label::new(Some(empty_text));
        let stack = gtk::Stack::builder().hhomogeneous(false).build();
        stack.add_named(&plain, Some("plain"));
        stack.add_named(&shortcut, Some("shortcut"));
        stack.set_visible_child_name("plain");
        let button = gtk::Button::builder()
            .child(&stack)
            .valign(gtk::Align::Center)
            .build();
        let this = Self {
            button,
            stack,
            shortcut,
            plain,
            recorded: Rc::default(),
        };
        let recorded = Rc::clone(&this.recorded);
        this.button.connect_clicked(move |button| {
            let recorded = Rc::clone(&recorded);
            record(button, "Enter the new shortcut", false, move |outcome| {
                let callback = recorded.borrow().clone();
                if let Some(callback) = callback {
                    callback(&outcome);
                }
            });
        });
        this
    }

    /// The button, for a row's suffix.
    #[must_use]
    pub fn widget(&self) -> &gtk::Button {
        &self.button
    }

    /// Show `stored`, or the empty text for `None`.
    pub fn set_binding(&self, stored: Option<&str>) {
        let accelerator = stored
            .and_then(accelerator)
            .filter(|accel| gtk::accelerator_parse(accel).is_some());
        match (stored, accelerator) {
            (Some(_), Some(accelerator)) => {
                self.shortcut.set_accelerator(&accelerator);
                self.stack.set_visible_child_name("shortcut");
            }
            (Some(stored), None) => {
                self.plain.set_label(&label(stored));
                self.stack.set_visible_child_name("plain");
            }
            (None, _) => self.stack.set_visible_child_name("plain"),
        }
    }

    /// Run `recorded` with what the user did in the dialog.
    pub fn connect_recorded(&self, recorded: impl Fn(&Recorded) + 'static) {
        self.recorded.replace(Some(Rc::new(recorded)));
    }
}

type Added = dyn Fn(&str);
type Removed = dyn Fn(usize);

#[derive(Default)]
struct ChipHandlers {
    added: Option<Rc<Added>>,
    removed: Option<Rc<Removed>>,
}

/// Several bindings as removable chips, then "+" (KEY-02).
#[derive(Clone)]
pub struct ShortcutChips {
    widget: gtk::Box,
    chips: gtk::Box,
    handlers: Rc<RefCell<ChipHandlers>>,
}

impl std::fmt::Debug for ShortcutChips {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShortcutChips")
            .field("widget", &self.widget)
            .finish_non_exhaustive()
    }
}

impl Default for ShortcutChips {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutChips {
    /// No chips, and the "+" button.
    #[must_use]
    pub fn new() -> Self {
        let chips = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();
        let add = gtk::Button::builder()
            .css_classes(["flat", "circular"])
            .icon_name("list-add-symbolic")
            .tooltip_text("Add Shortcut")
            .valign(gtk::Align::Center)
            .build();
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        widget.append(&chips);
        widget.append(&add);
        let handlers: Rc<RefCell<ChipHandlers>> = Rc::default();
        let on_add = Rc::clone(&handlers);
        add.connect_clicked(move |add| {
            let on_add = Rc::clone(&on_add);
            record(add, "Enter the new shortcut", false, move |outcome| {
                if let Recorded::Binding { stored, .. } = outcome {
                    let added = on_add.borrow().added.clone();
                    if let Some(added) = added {
                        added(&stored);
                    }
                }
            });
        });
        Self {
            widget,
            chips,
            handlers,
        }
    }

    /// The chips and "+", for a row's suffix.
    #[must_use]
    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    /// Show `stored`, one chip each.
    pub fn set_bindings(&self, stored: &[String]) {
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        for (index, binding) in stored.iter().enumerate() {
            let text = gtk::Label::new(Some(&label(binding)));
            let remove = gtk::Button::builder()
                .css_classes(["flat", "circular"])
                .icon_name("window-close-symbolic")
                .tooltip_text(format!("Remove {}", label(binding)))
                .build();
            let handlers = Rc::clone(&self.handlers);
            remove.connect_clicked(move |_| {
                let removed = handlers.borrow().removed.clone();
                if let Some(removed) = removed {
                    removed(index);
                }
            });
            let chip = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(2)
                .valign(gtk::Align::Center)
                .css_classes(["wye-shortcut-chip"])
                .build();
            chip.append(&text);
            chip.append(&remove);
            self.chips.append(&chip);
        }
    }

    /// Run `added(stored)` after the user records another binding.
    pub fn connect_added(&self, added: impl Fn(&str) + 'static) {
        self.handlers.borrow_mut().added = Some(Rc::new(added));
    }

    /// Run `removed(index)` when a chip's remove button is pressed.
    pub fn connect_removed(&self, removed: impl Fn(usize) + 'static) {
        self.handlers.borrow_mut().removed = Some(Rc::new(removed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_02_a_combination_is_stored_with_xkb_names() {
        let outcome = interpret(
            gdk::Key::O,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
            false,
        );
        assert_eq!(
            outcome,
            Some(Recorded::Binding {
                stored: "Ctrl+Shift+o".to_owned(),
                label: "Ctrl+Shift+O".to_owned(),
            })
        );
    }

    #[test]
    fn key_02_escape_cancels_backspace_clears_modifiers_wait() {
        let none = gdk::ModifierType::empty();
        assert_eq!(
            interpret(gdk::Key::Escape, none, false),
            Some(Recorded::Cancel)
        );
        assert_eq!(
            interpret(gdk::Key::BackSpace, none, false),
            Some(Recorded::Clear)
        );
        assert_eq!(interpret(gdk::Key::Control_L, none, false), None);
    }

    #[test]
    fn key_12_a_single_key_ignores_modifiers() {
        let outcome = interpret(gdk::Key::f, gdk::ModifierType::CONTROL_MASK, true);
        assert!(matches!(outcome, Some(Recorded::Binding { stored, .. }) if stored == "f"));
    }

    #[test]
    fn stored_bindings_become_accelerators() {
        assert_eq!(
            accelerator("Ctrl+Alt+o").as_deref(),
            Some("<Control><Alt>o")
        );
        assert_eq!(accelerator("Return").as_deref(), Some("Return"));
        assert_eq!(accelerator("Hyper+o"), None);
        assert_eq!(accelerator(""), None);
    }
}
