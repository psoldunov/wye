//! BLK-15 Reorderable checklist: rows with a check box, an icon, a name, an
//! optional small control (the shown browsers' hotkey popup) and, on checked
//! rows, a drag handle (`list-drag-handle-symbolic`). Dragging a checked row
//! onto another checked row moves it there; so do Alt+Up and Alt+Down on a
//! checked row, which keep the focus on it so the keys can be pressed again.
//!
//! The list is rebuilt from data ([`Checklist::set_items`]); it never changes
//! its own order or checks. The page saves the change through the store and
//! the store's `changed` brings the new items back, so what is shown is
//! always what is saved. The keyboard focus stays on the row (and the check
//! box) it was on, by key, wherever that row lands.
//!
//! API:
//! - [`Checklist::new`]`()`: an empty `boxed-list`; add [`Checklist::widget`]
//!   to a group.
//! - [`Checklist::set_items`]`(items)`: show `items` in order.
//! - [`Checklist::connect_toggled`]`(|key, checked|)`: a check box flipped.
//! - [`Checklist::connect_moved`]`(|from, to|)`: a checked row was dropped
//!   on another, or moved with Alt+Up/Down; indices are positions in the
//!   items last set.
//! - [`ChecklistItem::extra`]: build the row's small control per item.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};
use serde_json::Value;

use super::icon;

/// One row of a checklist.
#[derive(Debug, Clone)]
pub struct ChecklistItem {
    /// What the callbacks name the row by (a target's JSON, an ID).
    pub key: String,
    pub title: String,
    /// Dimmed text under the title; empty for none.
    pub subtitle: String,
    /// Icon theme name or path; empty for none.
    pub icon: String,
    /// The profile badge (`{"initial", "color"}` or `{"image"}`).
    pub badge: Option<Value>,
    pub checked: bool,
    /// A control before the drag handle (a hotkey popup).
    pub extra: Option<gtk::Widget>,
}

type Toggled = dyn Fn(&str, bool);
type Moved = dyn Fn(usize, usize);

#[derive(Default)]
struct Handlers {
    toggled: Option<Rc<Toggled>>,
    moved: Option<Rc<Moved>>,
}

/// Where the keyboard focus was in a row, to put it back after a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    /// On the row itself.
    Row,
    /// On its check box.
    Check,
}

/// A checklist. Clones share the list.
#[derive(Clone)]
pub struct Checklist {
    list: gtk::ListBox,
    handlers: Rc<RefCell<Handlers>>,
    /// The rows shown, by key, with their check boxes.
    rows: Rc<RefCell<Vec<(String, adw::ActionRow, gtk::CheckButton)>>>,
}

impl std::fmt::Debug for Checklist {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Checklist")
            .field("list", &self.list)
            .finish_non_exhaustive()
    }
}

impl Default for Checklist {
    fn default() -> Self {
        Self::new()
    }
}

impl Checklist {
    /// An empty checklist.
    #[must_use]
    pub fn new() -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        Self {
            list,
            handlers: Rc::default(),
            rows: Rc::default(),
        }
    }

    /// The list, to add to a group.
    #[must_use]
    pub fn widget(&self) -> &gtk::ListBox {
        &self.list
    }

    /// Run `toggled(key, checked)` when a check box flips.
    pub fn connect_toggled(&self, toggled: impl Fn(&str, bool) + 'static) {
        self.handlers.borrow_mut().toggled = Some(Rc::new(toggled));
    }

    /// Run `moved(from, to)` when a checked row is dropped on another.
    pub fn connect_moved(&self, moved: impl Fn(usize, usize) + 'static) {
        self.handlers.borrow_mut().moved = Some(Rc::new(moved));
    }

    /// Show `items`, in order. The focus stays on the row it was on.
    pub fn set_items(&self, items: &[ChecklistItem]) {
        let focused = self.focused();
        self.list.remove_all();
        let checked = items.iter().filter(|item| item.checked).count();
        let rows: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let (row, check) = self.row(index, item, checked);
                self.list.append(&row);
                (item.key.clone(), row, check)
            })
            .collect();
        self.rows.replace(rows);
        if let Some((key, spot)) = focused {
            self.focus(&key, spot);
        }
    }

    /// The check box of the row called `key`, for the kit's GTK test.
    #[cfg(test)]
    pub(crate) fn check_of(&self, key: &str) -> Option<gtk::CheckButton> {
        self.rows
            .borrow()
            .iter()
            .find(|(candidate, ..)| candidate == key)
            .map(|(.., check)| check.clone())
    }

    /// The key of the row the focus is in, and where in it.
    fn focused(&self) -> Option<(String, Spot)> {
        let focus = self.list.root()?.focus()?;
        self.rows.borrow().iter().find_map(|(key, row, check)| {
            if focus == *check.upcast_ref::<gtk::Widget>() {
                Some((key.clone(), Spot::Check))
            } else if focus.is_ancestor(row) || focus == *row.upcast_ref::<gtk::Widget>() {
                Some((key.clone(), Spot::Row))
            } else {
                None
            }
        })
    }

    /// Focus the row called `key`, or its check box.
    fn focus(&self, key: &str, spot: Spot) {
        let rows = self.rows.borrow();
        if let Some((_, row, check)) = rows.iter().find(|(candidate, ..)| candidate == key) {
            match spot {
                Spot::Row => row.grab_focus(),
                Spot::Check => check.grab_focus(),
            };
        }
    }

    fn row(
        &self,
        index: usize,
        item: &ChecklistItem,
        checked: usize,
    ) -> (adw::ActionRow, gtk::CheckButton) {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&item.title))
            .build();
        if !item.subtitle.is_empty() {
            row.set_subtitle(&glib::markup_escape_text(&item.subtitle));
        }
        let check = gtk::CheckButton::builder()
            .active(item.checked)
            .valign(gtk::Align::Center)
            .build();
        check.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        // `add_prefix` puts each prefix before the ones already there: the
        // icon first, then the check box ahead of it (BLK-15: check box,
        // icon, name).
        row.add_prefix(&icon::target_icon(&item.icon, item.badge.as_ref(), 24));
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        if let Some(extra) = &item.extra {
            extra.set_valign(gtk::Align::Center);
            row.add_suffix(extra);
        }
        let key = item.key.clone();
        let handlers = Rc::clone(&self.handlers);
        check.connect_toggled(move |check| {
            let toggled = handlers.borrow().toggled.clone();
            if let Some(toggled) = toggled {
                toggled(&key, check.is_active());
            }
        });
        if item.checked {
            let handle = gtk::Image::builder()
                .icon_name("list-drag-handle-symbolic")
                .tooltip_text("Drag, or press Alt+Up or Alt+Down, to reorder")
                .css_classes(["wye-drag-handle"])
                .build();
            row.add_suffix(&handle);
            self.make_draggable(&row, &handle, index);
            self.add_move_keys(&row, index, checked);
        } else {
            // The handle's room, so the controls before it line up with the
            // checked rows'.
            let room = gtk::Image::builder()
                .icon_name("list-drag-handle-symbolic")
                .opacity(0.0)
                .can_target(false)
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build();
            row.add_suffix(&room);
        }
        (row, check)
    }

    /// Alt+Up and Alt+Down move checked row `index` among the `checked`
    /// rows, which come first.
    fn add_move_keys(&self, row: &adw::ActionRow, index: usize, checked: usize) {
        let keys = gtk::EventControllerKey::new();
        let handlers = Rc::clone(&self.handlers);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(to) = move_target(index, checked, key, modifiers) else {
                return glib::Propagation::Proceed;
            };
            let moved = handlers.borrow().moved.clone();
            if let Some(moved) = moved {
                moved(index, to);
            }
            glib::Propagation::Stop
        });
        row.add_controller(keys);
    }

    fn make_draggable(&self, row: &adw::ActionRow, handle: &gtk::Image, index: usize) {
        let Ok(position) = u32::try_from(index) else {
            return;
        };
        let source = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .content(&gdk::ContentProvider::for_value(&position.to_value()))
            .build();
        source.connect_drag_begin(glib::clone!(
            #[weak]
            row,
            move |source, _| {
                let paintable = gtk::WidgetPaintable::new(Some(&row));
                source.set_icon(Some(&paintable), 0, 0);
            }
        ));
        handle.add_controller(source);

        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        target.connect_motion(glib::clone!(
            #[weak]
            row,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, y| {
                let above = y < f64::from(row.height()) / 2.0;
                mark_drop(&row, Some(above));
                gdk::DragAction::MOVE
            }
        ));
        target.connect_leave(glib::clone!(
            #[weak]
            row,
            move |_| mark_drop(&row, None)
        ));
        let handlers = Rc::clone(&self.handlers);
        target.connect_drop(glib::clone!(
            #[weak]
            row,
            #[upgrade_or]
            false,
            move |_, value, _, y| {
                mark_drop(&row, None);
                let Ok(from) = value.get::<u32>() else {
                    return false;
                };
                let above = y < f64::from(row.height()) / 2.0;
                let to = drop_index(from as usize, index, above);
                let moved = handlers.borrow().moved.clone();
                if let Some(moved) = moved
                    && to != from as usize
                {
                    moved(from as usize, to);
                }
                true
            }
        ));
        row.add_controller(target);
    }
}

/// Where Alt+Up or Alt+Down moves checked row `index` of `checked`, if the
/// key is one of them and the row can move that way.
fn move_target(
    index: usize,
    checked: usize,
    key: gdk::Key,
    modifiers: gdk::ModifierType,
) -> Option<usize> {
    // As the rule list reads it (RUL-04): Alt held, Shift or Ctrl not.
    let alt = modifiers.contains(gdk::ModifierType::ALT_MASK)
        && !modifiers.intersects(gdk::ModifierType::SHIFT_MASK | gdk::ModifierType::CONTROL_MASK);
    match key {
        gdk::Key::Up | gdk::Key::KP_Up if alt => index.checked_sub(1),
        gdk::Key::Down | gdk::Key::KP_Down if alt && index + 1 < checked => Some(index + 1),
        _ => None,
    }
}

/// Where a row dragged from `from` lands when dropped on the row at `onto`,
/// above or below its middle: an index in the list after the row is taken
/// out. The rule list drags the same way (RUL-04).
pub(crate) fn drop_index(from: usize, onto: usize, above: bool) -> usize {
    let slot = if above { onto } else { onto + 1 };
    if slot > from { slot - 1 } else { slot }
}

/// Draw the line a drop on `row` would land at: above it, below it, or
/// none.
pub(crate) fn mark_drop(row: &adw::ActionRow, above: Option<bool>) {
    row.remove_css_class("wye-drop-above");
    row.remove_css_class("wye-drop-below");
    match above {
        Some(true) => row.add_css_class("wye-drop-above"),
        Some(false) => row.add_css_class("wye-drop-below"),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use gtk::gdk;

    use super::{drop_index, move_target};

    #[test]
    fn blk_15_a_drop_lands_where_the_line_shows() {
        // Down: dropped below row 2 of [0, 1, 2, 3], row 0 becomes index 2.
        assert_eq!(drop_index(0, 2, false), 2);
        assert_eq!(drop_index(0, 2, true), 1);
        // Up: dropped above row 0, row 3 becomes index 0.
        assert_eq!(drop_index(3, 0, true), 0);
        assert_eq!(drop_index(3, 0, false), 1);
        // On itself: no move.
        assert_eq!(drop_index(1, 1, true), 1);
        assert_eq!(drop_index(1, 1, false), 1);
    }

    #[test]
    fn blk_15_alt_up_and_down_move_a_checked_row() {
        let alt = gdk::ModifierType::ALT_MASK;
        assert_eq!(move_target(1, 3, gdk::Key::Up, alt), Some(0));
        assert_eq!(move_target(1, 3, gdk::Key::Down, alt), Some(2));
        // Not past the first row, nor below the last checked one.
        assert_eq!(move_target(0, 3, gdk::Key::Up, alt), None);
        assert_eq!(move_target(2, 3, gdk::Key::Down, alt), None);
        // Without Alt, or with more than Alt, the keys move the focus.
        assert_eq!(
            move_target(1, 3, gdk::Key::Up, gdk::ModifierType::empty()),
            None
        );
        let alt_shift = alt | gdk::ModifierType::SHIFT_MASK;
        assert_eq!(move_target(1, 3, gdk::Key::Up, alt_shift), None);
    }
}
