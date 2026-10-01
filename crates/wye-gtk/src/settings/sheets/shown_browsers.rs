//! The shown browsers sheet, "Shown Browsers" (SHOWN-01 to SHOWN-08): which
//! targets the picker and the tray menu list, in which order, and with
//! which hotkey.
//!
//! A reorderable checklist (BLK-15): one row per candidate (every installed
//! browser, every profile as "<Profile> (<Browser>)", every app added with
//! "+", then private windows), checked rows first in the user's order with
//! a drag handle (SHOWN-02, SHOWN-03). Each row ends in its hotkey popup
//! (SHOWN-04): a grid of key caps (a to z and 0 to 9 without the keys the
//! picker's actions use, KEY-12), **None**, and **Other Key…**, which
//! records any single key. A hotkey is unique; choosing one another row has
//! moves it. Under a hotkey scheme other than "Assigned per browser"
//! (KEY-10) the popups are insensitive and show the key the scheme gives.
//! An app added with **Add App…** (SHOWN-05, the app chooser) has a remove
//! button (SHOWN-08). Every change applies at once; **Done** closes the
//! sheet (SHOWN-06). The list scrolls under the pinned header (SHOWN-07).
//!
//! The rows and every edit come from the shared model
//! (`crate::settings::shown`, `crate::settings::hotkeys`) through the store,
//! and the sheet rebuilds from the store after each change, so what it
//! shows is what is saved.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/ShownBrowsersSheet.qml and
//! ShownHotkeyChooser.qml.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;

use super::app_chooser::{AppChooser, Choice};
use crate::settings::hotkeys::Choice as KeyChoice;
use crate::settings::shown::{self, Row};
use crate::settings::store::SettingsStore;
use crate::widgets::checklist::{Checklist, ChecklistItem};
use crate::widgets::sheet::Sheet;
use crate::widgets::shortcut::{self, Recorded};
use crate::widgets::{empty_state, group, icon, row};

/// The list's description: what the checks and the order mean.
const LIST_NOTE: &str = "Checked browsers appear in the picker and the tray menu, in this order. Drag a checked browser to move it.";

/// Added under a hotkey scheme other than "Assigned per browser" (KEY-10).
const SCHEME_NOTE: &str = "The hotkeys follow the hotkey scheme set on the Picker page.";

/// Key caps per line of the hotkey popup.
const KEYS_PER_LINE: usize = 6;

/// Everything a rebuild depends on, to skip rebuilding an unchanged list.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    rows: Vec<Row>,
    per_browser: bool,
    choices: Vec<KeyChoice>,
}

struct Inner {
    sheet: Sheet,
    store: glib::WeakRef<SettingsStore>,
    checklist: Checklist,
    list: adw::PreferencesGroup,
    empty: adw::PreferencesGroup,
    shown: RefCell<Option<Shown>>,
    /// The rows' hotkey buttons, in order (the self-test opens one).
    hotkeys: RefCell<Vec<gtk::MenuButton>>,
    changed: RefCell<Option<glib::SignalHandlerId>>,
}

/// An open shown browsers sheet. Clones share it.
#[derive(Clone)]
pub struct ShownBrowsersSheet {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for ShownBrowsersSheet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShownBrowsersSheet")
            .finish_non_exhaustive()
    }
}

impl ShownBrowsersSheet {
    /// Build the sheet on `store` and show it over `parent`'s window
    /// (SHOWN-01).
    pub fn open(parent: &impl IsA<gtk::Widget>, store: &SettingsStore) -> Self {
        let this = Self::build(store);
        this.inner.sheet.present(parent);
        this
    }

    /// Open the app chooser over the sheet (SHOWN-05); `search` is typed
    /// into it.
    pub fn add_app(&self, search: &str) {
        let chooser = add_app(&self.inner);
        if let Some(chooser) = chooser
            && !search.is_empty()
        {
            chooser.set_search(search);
        }
    }

    /// Open the hotkey popup of the first row (self-test).
    pub fn open_hotkey(&self) {
        let first = self.inner.hotkeys.borrow().first().cloned();
        if let Some(button) = first {
            button.popup();
        }
    }

    fn build(store: &SettingsStore) -> Self {
        let sheet = Sheet::new("Shown Browsers", "Done");
        // SHOWN-06: every change is already saved; there is nothing to cancel.
        sheet.cancel().set_visible(false);
        sheet.set_valid(true);
        sheet.connect_primary(|| true);

        let checklist = Checklist::new();
        let list = group::group_with_description("", LIST_NOTE);
        list.add(checklist.widget());
        sheet.page().add(&list);
        let empty = group::group("");
        empty.add(&empty_state::empty_state(
            "wye-browsers-symbolic",
            "No Browsers Found",
            "Click Rescan on the Browsers page.",
        ));
        sheet.page().add(&empty);

        // SHOWN-05: Add App… after the list.
        let add = adw::ButtonRow::builder()
            .title("Add App…")
            .start_icon_name("list-add-symbolic")
            .build();
        let actions = group::group("");
        actions.add(&add);
        sheet.page().add(&actions);
        row::follow_writable(store, checklist.widget(), |_| true);
        row::follow_writable(store, &add, |_| true);

        let inner = Rc::new(Inner {
            sheet,
            store: store.downgrade(),
            checklist,
            list,
            empty,
            shown: RefCell::default(),
            hotkeys: RefCell::default(),
            changed: RefCell::default(),
        });
        connect(&inner, &add);
        show(&inner, store);
        let weak = Rc::downgrade(&inner);
        let handler = store.connect_changed(move |store| {
            if let Some(inner) = weak.upgrade() {
                show(&inner, store);
            }
        });
        inner.changed.replace(Some(handler));
        // The sheet keeps itself until it closes, then lets go of the store.
        let keep = RefCell::new(Some(Rc::clone(&inner)));
        inner.sheet.dialog().connect_closed(move |_| {
            if let Some(inner) = keep.take()
                && let (Some(store), Some(handler)) = (inner.store.upgrade(), inner.changed.take())
            {
                store.disconnect(handler);
            }
        });
        Self { inner }
    }
}

/// The checklist's and the add button's edits, saved through the store.
fn connect(inner: &Rc<Inner>, add: &adw::ButtonRow) {
    let weak = Rc::downgrade(inner);
    inner.checklist.connect_toggled(glib::clone!(
        #[strong]
        weak,
        move |key, checked| {
            edit(&weak, |inner, entries| {
                target_of(inner, key).map(|target| shown::toggle(entries, &target, checked))
            });
        }
    ));
    inner.checklist.connect_moved(glib::clone!(
        #[strong]
        weak,
        move |from, to| {
            edit(&weak, |_, entries| {
                Some(shown::move_entry(entries, from, to))
            });
        }
    ));
    add.connect_activated(move |_| {
        if let Some(inner) = weak.upgrade() {
            add_app(&inner);
        }
    });
}

/// Save what `change` makes of the shown list; `None` changes nothing.
fn edit(
    weak: &Weak<Inner>,
    change: impl FnOnce(&Inner, &[shown::Entry]) -> Option<Vec<shown::Entry>>,
) {
    let Some(inner) = weak.upgrade() else {
        return;
    };
    let Some(store) = inner.store.upgrade() else {
        return;
    };
    store.edit_shown(|entries| change(&inner, entries).unwrap_or_else(|| entries.to_vec()));
}

/// The target of the row whose key is `key`.
fn target_of(inner: &Inner, key: &str) -> Option<Value> {
    inner.shown.borrow().as_ref().and_then(|shown| {
        shown
            .rows
            .iter()
            .find(|row| row.key == key)
            .map(|row| row.target.clone())
    })
}

/// SHOWN-05: choose an app to add, checked, at the end of the list.
fn add_app(inner: &Rc<Inner>) -> Option<AppChooser> {
    let store = inner.store.upgrade()?;
    let weak = Rc::downgrade(inner);
    let chooser = AppChooser::open(
        inner.sheet.dialog(),
        &store,
        Choice::Single,
        move |targets| {
            edit(&weak, |_, entries| {
                Some(
                    targets
                        .iter()
                        .fold(entries.to_vec(), |list, target| shown::add(&list, target)),
                )
            });
        },
    );
    Some(chooser)
}

/// Rebuild the list from the store, unless nothing it shows changed.
fn show(inner: &Rc<Inner>, store: &SettingsStore) {
    let next = Shown {
        rows: store.shown_rows(),
        per_browser: store.hotkeys_per_browser(),
        choices: store.hotkey_choices(),
    };
    if inner.shown.borrow().as_ref() == Some(&next) {
        return;
    }
    let empty = next.rows.is_empty();
    inner.list.set_visible(!empty);
    inner.list.set_description(Some(&if next.per_browser {
        LIST_NOTE.to_owned()
    } else {
        format!("{LIST_NOTE} {SCHEME_NOTE}")
    }));
    inner.empty.set_visible(empty);
    // The remove button's room is kept on every row once any row has one,
    // so the hotkey popups line up (SHOWN-08).
    let any_removable = next.rows.iter().any(|row| row.removable && row.checked);
    let used = used_keys(&next);
    let mut buttons = Vec::new();
    let items: Vec<ChecklistItem> = next
        .rows
        .iter()
        .map(|row| {
            let (extra, button) = trailing(inner, &next, row, &used, any_removable);
            buttons.push(button);
            ChecklistItem {
                key: row.key.clone(),
                title: row.name.clone(),
                subtitle: if row.missing {
                    "No longer installed".to_owned()
                } else {
                    String::new()
                },
                icon: if row.icon.is_empty() {
                    icon::FALLBACK_APP_ICON.to_owned()
                } else {
                    row.icon.clone()
                },
                badge: row.badge.clone(),
                checked: row.checked,
                extra: Some(extra),
            }
        })
        .collect();
    inner.shown.replace(Some(next));
    inner.checklist.set_items(&items);
    inner.hotkeys.replace(buttons);
}

/// Which row has each stored hotkey, by name (SHOWN-04: a key in use moves).
fn used_keys(shown: &Shown) -> BTreeMap<String, String> {
    shown
        .rows
        .iter()
        .filter_map(|row| Some((row.hotkey.clone()?, row.name.clone())))
        .collect()
}

/// A row's trailing controls: its hotkey popup and, for an added app, the
/// remove button.
fn trailing(
    inner: &Rc<Inner>,
    shown: &Shown,
    row: &Row,
    used: &BTreeMap<String, String>,
    any_removable: bool,
) -> (gtk::Widget, gtk::MenuButton) {
    let controls = gtk::Box::builder().spacing(6).build();
    let button = hotkey_button(inner, shown, row, used);
    controls.append(&button);
    if any_removable && !(row.removable && row.checked) {
        // The room of a remove button, so the hotkey popups line up; not a
        // button to screen readers, the pointer or the focus.
        let room = gtk::Image::builder()
            .icon_name("user-trash-symbolic")
            .opacity(0.0)
            .can_target(false)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .css_classes(["wye-remove-room"])
            .build();
        controls.append(&room);
    } else if any_removable {
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove")
            .valign(gtk::Align::Center)
            .build();
        remove.add_css_class("flat");
        remove.add_css_class("circular");
        remove.update_property(&[gtk::accessible::Property::Label(&format!(
            "Remove {}",
            row.name
        ))]);
        let target = row.target.clone();
        let weak = Rc::downgrade(inner);
        remove.connect_clicked(move |_| {
            edit(&weak, |_, entries| Some(shown::remove(entries, &target)));
        });
        controls.append(&remove);
    }
    (controls.upcast(), button)
}

/// SHOWN-04: the row's hotkey, opening the key grid.
fn hotkey_button(
    inner: &Rc<Inner>,
    shown: &Shown,
    row: &Row,
    used: &BTreeMap<String, String>,
) -> gtk::MenuButton {
    let key = if shown.per_browser {
        row.hotkey.as_deref().map(shortcut::label)
    } else {
        row.shown_hotkey.clone()
    };
    let label = gtk::Label::new(Some(key.as_deref().unwrap_or("None")));
    if key.is_none() {
        label.add_css_class("dimmed");
    }
    let button = gtk::MenuButton::builder()
        .child(&label)
        .always_show_arrow(true)
        .valign(gtk::Align::Center)
        .sensitive(shown.per_browser)
        .tooltip_text(if shown.per_browser {
            "Hotkey in the picker"
        } else {
            "The picker's hotkey scheme gives this key"
        })
        .build();
    button.add_css_class("wye-hotkey-button");
    // SHOWN-04: named for screen readers by what it sets, not only its key.
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Hotkey for {}: {}",
        row.name,
        key.as_deref().unwrap_or("None")
    ))]);
    if shown.per_browser {
        let weak = Rc::downgrade(inner);
        let choices = shown.choices.clone();
        let row = row.clone();
        let used = used.clone();
        // Built when opened: a sheet lists many rows, and few popups open.
        button.set_create_popup_func(move |button| {
            if button.popover().is_none() {
                let popover = key_grid(&weak, &row, &choices, &used, button);
                button.set_popover(Some(&popover));
            }
        });
    }
    button
}

/// The popup: a grid of key caps, then **None** and **Other Key…**.
fn key_grid(
    weak: &Weak<Inner>,
    row: &Row,
    choices: &[KeyChoice],
    used: &BTreeMap<String, String>,
    button: &gtk::MenuButton,
) -> gtk::Popover {
    let weak = weak.clone();
    let grid = gtk::Grid::builder()
        .row_spacing(4)
        .column_spacing(4)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .build();
    let popover = gtk::Popover::new();
    for (index, choice) in choices.iter().enumerate() {
        let cap = key_cap(&weak, row, choice, used.get(&choice.key), &popover);
        let (column, line) = (index % KEYS_PER_LINE, index / KEYS_PER_LINE);
        // At most 36 keys: the positions fit an i32.
        let (Ok(column), Ok(line)) = (i32::try_from(column), i32::try_from(line)) else {
            continue;
        };
        grid.attach(&cap, column, line, 1, 1);
    }
    let none = gtk::Button::with_label("None");
    none.add_css_class("flat");
    let other = gtk::Button::with_label("Other Key…");
    other.add_css_class("flat");
    let footer = gtk::Box::builder().homogeneous(true).spacing(4).build();
    footer.append(&none);
    footer.append(&other);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    content.append(&grid);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&footer);
    popover.set_child(Some(&content));
    // Added, not set: a popover's own `background` class must stay.
    popover.add_css_class("wye-hotkey-popover");

    let target = row.target.clone();
    none.connect_clicked(glib::clone!(
        #[strong]
        weak,
        #[strong]
        target,
        #[weak]
        popover,
        move |_| {
            popover.popdown();
            set_hotkey(&weak, &target, None);
        }
    ));
    let anchor = button.downgrade();
    let weak = weak.clone();
    other.connect_clicked(glib::clone!(
        #[weak]
        popover,
        move |_| {
            popover.popdown();
            if let Some(anchor) = anchor.upgrade() {
                record_key(&weak, &target, &anchor);
            }
        }
    ));
    popover
}

/// One key cap; the row's own key is highlighted, a key another row has
/// says so in its tooltip.
fn key_cap(
    weak: &Weak<Inner>,
    row: &Row,
    choice: &KeyChoice,
    owner: Option<&String>,
    popover: &gtk::Popover,
) -> gtk::Button {
    let cap = gtk::Button::with_label(&choice.label);
    cap.add_css_class("wye-key-cap");
    if row.hotkey.as_deref() == Some(choice.key.as_str()) {
        cap.add_css_class("suggested-action");
    } else if let Some(owner) = owner {
        cap.add_css_class("wye-key-cap-used");
        cap.set_tooltip_text(Some(&format!("Used by {owner}; choosing it moves it here")));
    }
    let target = row.target.clone();
    let key = choice.key.clone();
    let weak = weak.clone();
    cap.connect_clicked(glib::clone!(
        #[weak]
        popover,
        move |_| {
            popover.popdown();
            set_hotkey(&weak, &target, Some(&key));
        }
    ));
    cap
}

/// Give `target` the hotkey `key` (none for `None`); the row that had it
/// loses it, an unchecked row is checked (SHOWN-04).
fn set_hotkey(weak: &Weak<Inner>, target: &Value, key: Option<&str>) {
    edit(weak, |_, entries| {
        Some(shown::set_hotkey(entries, target, key))
    });
}

/// **Other Key…**: record one key; a key a picker action uses is refused
/// with the reason (KEY-12).
fn record_key(weak: &Weak<Inner>, target: &Value, anchor: &gtk::MenuButton) {
    let weak = weak.clone();
    let target = target.clone();
    let parent = anchor.downgrade();
    shortcut::record(
        anchor,
        "Press the key for this browser",
        true,
        move |recorded| match recorded {
            Recorded::Cancel => {}
            Recorded::Clear => set_hotkey(&weak, &target, None),
            Recorded::Binding { stored, .. } => {
                let Some(store) = weak.upgrade().and_then(|inner| inner.store.upgrade()) else {
                    return;
                };
                match store.check_hotkey(&stored) {
                    Ok(key) => set_hotkey(&weak, &target, Some(&key)),
                    Err(error) => {
                        if let Some(parent) = parent.upgrade() {
                            refuse(&parent, &error.to_string());
                        }
                    }
                }
            }
        },
    );
}

/// Say why a recorded key cannot be a hotkey.
fn refuse(parent: &gtk::MenuButton, reason: &str) {
    let alert = adw::AlertDialog::builder()
        .heading("Cannot Use This Key")
        .body(reason)
        .build();
    alert.add_response("ok", "OK");
    alert.set_default_response(Some("ok"));
    alert.present(Some(parent));
}
