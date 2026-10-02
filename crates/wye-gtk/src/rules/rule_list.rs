//! The Rules page's list (RUL-03 to RUL-07): one `AdwActionRow` per rule in
//! a `boxed-list`, in the order the pipeline checks them. Each row: a drag
//! handle; the name with the dimmed one-line summary under it
//! ("github.com, gitlab.com · from Slack · Shift"); at the right the
//! target's icon and name in a column of one width (the widest name, up to
//! KDE's ten grid units), a switch that turns the rule off, and a delete
//! button. A turned-off rule is dimmed; its switch
//! and buttons are not.
//!
//! Clicking a row (or Return/Space on it) edits it (RUL-05); right-click,
//! the Menu key or Shift+F10 opens a menu (Edit…, Duplicate, Move Up, Move
//! Down, Delete); Delete deletes the focused row; Alt+Up and Alt+Down move
//! it and keep the focus on it, so the keys can be pressed again (RUL-04,
//! RUL-06).
//!
//! The list never changes the rules itself: it reports what the user did
//! and shows what the store holds afterwards ([`RuleList::show`]). When
//! only values changed (a switch, a move), the rows are updated in place,
//! so the keyboard focus stays where it was.
//!
//! KDE counterpart: crates/wye-ui/qml/rules/RuleList.qml.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use super::list::RowView;
use crate::settings::menu::Row as TargetLabel;
use crate::widgets::checklist::{drop_index, mark_drop};
use crate::widgets::icon;
use crate::widgets::section::clear_rows;

/// The longest target name the column shows whole, in characters (about
/// KDE's ten grid units); a longer one is cut. The column is as wide as the
/// widest name in it, the same in every row so the icons line up.
const TARGET_MAX_CHARS: i32 = 20;
/// The target's icon size.
const ICON_SIZE: i32 = 24;
/// The icon of a target that has none.
const NO_ICON: &str = "application-x-executable";

/// One rule as the list shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub view: RowView,
    /// The target's name and icon (TGT-01).
    pub target: TargetLabel,
}

type Index = dyn Fn(usize);
type Toggle = dyn Fn(usize, bool);
type Moved = dyn Fn(usize, usize);
/// A row menu action on the rule at an index.
type RowAction = fn(&State, usize);

#[derive(Default)]
struct Handlers {
    edit: Option<Rc<Index>>,
    toggle: Option<Rc<Toggle>>,
    delete: Option<Rc<Index>>,
    duplicate: Option<Rc<Index>>,
    moved: Option<Rc<Moved>>,
}

/// The widgets of one row that change with its rule.
#[derive(Debug, Clone)]
struct RowWidgets {
    row: adw::ActionRow,
    handle: gtk::Image,
    target: gtk::Box,
    switch: gtk::Switch,
    delete: gtk::Button,
    actions: gio::SimpleActionGroup,
}

#[derive(Default)]
struct State {
    items: RefCell<Vec<Item>>,
    rows: RefCell<Vec<RowWidgets>>,
    editable: Cell<bool>,
    /// Set while code sets a switch, so it is not reported as the user's.
    updating: Cell<bool>,
    /// The row to focus once the list shows the next change (a move).
    refocus: Cell<Option<usize>>,
    handlers: RefCell<Handlers>,
}

impl State {
    fn edit(&self, index: usize) {
        let handler = self.handlers.borrow().edit.clone();
        if let Some(handler) = handler {
            handler(index);
        }
    }

    fn toggle(&self, index: usize, on: bool) {
        let handler = self.handlers.borrow().toggle.clone();
        if let (Some(handler), true) = (handler, self.editable.get()) {
            handler(index, on);
        }
    }

    fn delete(&self, index: usize) {
        let handler = self.handlers.borrow().delete.clone();
        if let (Some(handler), true) = (handler, self.editable.get()) {
            handler(index);
        }
    }

    fn duplicate(&self, index: usize) {
        let handler = self.handlers.borrow().duplicate.clone();
        if let (Some(handler), true) = (handler, self.editable.get()) {
            handler(index);
        }
    }

    /// Move the rule at `from` to `to`; the focus follows it.
    fn shift(&self, from: usize, to: usize) {
        let count = self.items.borrow().len();
        if !self.editable.get() || from == to || to >= count {
            return;
        }
        let handler = self.handlers.borrow().moved.clone();
        if let Some(handler) = handler {
            self.refocus.set(Some(to));
            handler(from, to);
        }
    }

    /// Draw the drop line on row `spot` (index, above), or on none.
    fn mark(&self, spot: Option<(usize, bool)>) {
        for (index, widgets) in self.rows.borrow().iter().enumerate() {
            let above = spot.filter(|(at, _)| *at == index).map(|(_, above)| above);
            mark_drop(&widgets.row, above);
        }
    }
}

/// The rule list. Clones share it.
#[derive(Clone)]
pub struct RuleList {
    list: gtk::ListBox,
    menu: gio::Menu,
    state: Rc<State>,
    /// Gives every row's target column the width of the widest.
    targets: gtk::SizeGroup,
}

impl std::fmt::Debug for RuleList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuleList")
            .field("list", &self.list)
            .finish_non_exhaustive()
    }
}

impl RuleList {
    /// Fill `list` (the list card's `boxed-list`) with rules.
    #[must_use]
    pub fn new(list: &gtk::ListBox) -> Self {
        let this = Self {
            list: list.clone(),
            menu: row_menu(),
            state: Rc::default(),
            targets: gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal),
        };
        accept_drops(&this.list, &this.state);
        this
    }

    /// Run `edit(index)` when a rule is clicked or Edit… is chosen (RUL-05).
    pub fn connect_edit(&self, edit: impl Fn(usize) + 'static) {
        self.state.handlers.borrow_mut().edit = Some(Rc::new(edit));
    }

    /// Run `toggle(index, on)` when a rule's switch is flipped (RUL-07).
    pub fn connect_toggle(&self, toggle: impl Fn(usize, bool) + 'static) {
        self.state.handlers.borrow_mut().toggle = Some(Rc::new(toggle));
    }

    /// Run `delete(index)` for the delete button, Delete, or the menu's
    /// Delete (RUL-06).
    pub fn connect_delete(&self, delete: impl Fn(usize) + 'static) {
        self.state.handlers.borrow_mut().delete = Some(Rc::new(delete));
    }

    /// Run `duplicate(index)` for the menu's Duplicate.
    pub fn connect_duplicate(&self, duplicate: impl Fn(usize) + 'static) {
        self.state.handlers.borrow_mut().duplicate = Some(Rc::new(duplicate));
    }

    /// Run `moved(from, to)` after a drag, Move Up/Down or Alt+Up/Down
    /// (RUL-04).
    pub fn connect_moved(&self, moved: impl Fn(usize, usize) + 'static) {
        self.state.handlers.borrow_mut().moved = Some(Rc::new(moved));
    }

    /// Show `items`; `editable` is false for a read-only configuration (no
    /// dragging, switching or deleting).
    pub fn show(&self, items: Vec<Item>, editable: bool) {
        let state = &self.state;
        if *state.items.borrow() == items && state.editable.get() == editable {
            return;
        }
        state.editable.set(editable);
        let same_count = state.rows.borrow().len() == items.len();
        if same_count {
            for (index, item) in items.iter().enumerate() {
                let widgets = state.rows.borrow().get(index).cloned();
                if let Some(widgets) = widgets {
                    self.fill(&widgets, index, item, items.len());
                }
            }
        } else {
            self.rebuild(&items);
        }
        state.items.replace(items);
        if let Some(index) = state.refocus.take() {
            self.focus_row(index);
        }
    }

    /// Open row `index`'s menu (the self-test's view of it).
    pub fn open_menu(&self, index: usize) {
        let row = self.state.rows.borrow().get(index).map(|w| w.row.clone());
        if let Some(row) = row {
            popup(&row, &self.menu, None);
        }
    }

    fn rebuild(&self, items: &[Item]) {
        // The focused row's place, to give the focus back near it.
        let focused = self
            .list
            .focus_child()
            .and_downcast::<gtk::ListBoxRow>()
            .and_then(|row| usize::try_from(row.index()).ok());
        clear_rows(&self.list);
        let rows: Vec<RowWidgets> = (0..items.len()).map(|index| self.build(index)).collect();
        for (index, (widgets, item)) in rows.iter().zip(items).enumerate() {
            self.fill(widgets, index, item, items.len());
            self.list.append(&widgets.row);
        }
        self.state.rows.replace(rows);
        if let Some(index) = focused.filter(|_| !items.is_empty()) {
            self.focus_row(index.min(items.len() - 1));
        }
    }

    fn focus_row(&self, index: usize) {
        let row = self.state.rows.borrow().get(index).map(|w| w.row.clone());
        if let Some(row) = row {
            row.grab_focus();
        }
    }

    /// The row at `index`, without its rule yet ([`Self::fill`]).
    fn build(&self, index: usize) -> RowWidgets {
        let row = adw::ActionRow::builder()
            .activatable(true)
            .title_lines(1)
            .subtitle_lines(1)
            .build();
        row.add_css_class("wye-rule-row");
        let handle = gtk::Image::builder()
            .icon_name("list-drag-handle-symbolic")
            .tooltip_text("Drag to reorder")
            .css_classes(["wye-drag-handle"])
            .build();
        row.add_prefix(&handle);
        let target = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            // Not the name's expansion: the title takes the spare room.
            .hexpand(false)
            .valign(gtk::Align::Center)
            .css_classes(["wye-rule-target"])
            .build();
        self.targets.add_widget(&target);
        row.add_suffix(&target);
        let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        row.add_suffix(&switch);
        let delete = gtk::Button::builder()
            .css_classes(["flat", "circular", "wye-rule-delete"])
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete Rule")
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&delete);
        let actions = gio::SimpleActionGroup::new();
        row.insert_action_group("rule", Some(&actions));

        let state = &self.state;
        row.connect_activated(glib::clone!(
            #[weak]
            state,
            move |_| state.edit(index)
        ));
        switch.connect_active_notify(glib::clone!(
            #[weak]
            state,
            move |switch| {
                if !state.updating.get() {
                    state.toggle(index, switch.is_active());
                }
            }
        ));
        delete.connect_clicked(glib::clone!(
            #[weak]
            state,
            move |_| state.delete(index)
        ));
        add_actions(&actions, state, index);
        self.add_keys(&row, index);
        self.add_context_click(&row);
        drag_from(&handle, &row, index);
        RowWidgets {
            row,
            handle,
            target,
            switch,
            delete,
            actions,
        }
    }

    /// Show `item` in the row at `index` of `count`.
    fn fill(&self, widgets: &RowWidgets, index: usize, item: &Item, count: usize) {
        let view = &item.view;
        let editable = self.state.editable.get();
        let row = &widgets.row;
        row.set_title(&glib::markup_escape_text(&view.name));
        row.set_subtitle(&glib::markup_escape_text(&view.summary));
        // The stage and screen readers find a rule by its name (RUL-05).
        // Through `GtkWidget`: the binding's `AdwActionRow` names
        // `GtkAccessible` only under a newer feature.
        row.upcast_ref::<gtk::Widget>().update_property(&[
            gtk::accessible::Property::Label(&view.name),
            gtk::accessible::Property::Description(&view.summary),
        ]);
        if view.enabled {
            row.remove_css_class("wye-rule-off");
        } else {
            row.add_css_class("wye-rule-off");
        }
        widgets.handle.set_sensitive(editable && count > 1);
        show_target(&widgets.target, &item.target);

        let switch = &widgets.switch;
        self.state.updating.set(true);
        switch.set_active(view.enabled);
        self.state.updating.set(false);
        switch.set_sensitive(editable);
        switch.set_tooltip_text(Some(if view.enabled {
            "Turn this rule off"
        } else {
            "Turn this rule on"
        }));
        switch.update_property(&[gtk::accessible::Property::Label(&format!(
            "Use “{}”",
            view.name
        ))]);
        widgets.delete.set_sensitive(editable);
        widgets
            .delete
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Delete “{}”",
                view.name
            ))]);
        for (name, enabled) in [
            ("duplicate", editable),
            ("move-up", editable && index > 0),
            ("move-down", editable && index + 1 < count),
            ("delete", editable),
        ] {
            if let Some(action) = widgets
                .actions
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(enabled);
            }
        }
    }

    /// Delete, the Menu key, Shift+F10, Alt+Up and Alt+Down on a row.
    fn add_keys(&self, row: &adw::ActionRow, index: usize) {
        let keys = gtk::EventControllerKey::new();
        let menu = self.menu.clone();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = state)]
            self.state,
            #[weak]
            row,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
                let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                match key {
                    gdk::Key::Delete | gdk::Key::KP_Delete => state.delete(index),
                    gdk::Key::Menu => popup(&row, &menu, None),
                    gdk::Key::F10 if shift => popup(&row, &menu, None),
                    gdk::Key::Up | gdk::Key::KP_Up if alt && index > 0 => {
                        state.shift(index, index - 1);
                    }
                    gdk::Key::Down | gdk::Key::KP_Down if alt => state.shift(index, index + 1),
                    _ => return glib::Propagation::Proceed,
                }
                glib::Propagation::Stop
            }
        ));
        row.add_controller(keys);
    }

    /// Right-click opens the row's menu where it was clicked.
    fn add_context_click(&self, row: &adw::ActionRow) {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        let menu = self.menu.clone();
        click.connect_pressed(glib::clone!(
            #[weak]
            row,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                popup(&row, &menu, Some((x, y)));
            }
        ));
        row.add_controller(click);
    }
}

/// The target column: icon and name; a long name is cut, its tooltip has it
/// whole.
fn show_target(target: &gtk::Box, label: &TargetLabel) {
    while let Some(child) = target.first_child() {
        target.remove(&child);
    }
    let source = if label.icon.is_empty() {
        NO_ICON
    } else {
        label.icon.as_str()
    };
    target.append(&icon::target_icon(source, label.badge.as_ref(), ICON_SIZE));
    // The whole name up to the column's limit, then cut.
    let name = gtk::Label::builder()
        .label(&label.label)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(TARGET_MAX_CHARS)
        .tooltip_text(&label.label)
        .build();
    target.append(&name);
}

/// The menu every row opens (RUL-04, RUL-06).
fn row_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let open = gio::Menu::new();
    open.append(Some("Edit…"), Some("rule.edit"));
    open.append(Some("Duplicate"), Some("rule.duplicate"));
    menu.append_section(None, &open);
    let order = gio::Menu::new();
    order.append(Some("Move Up"), Some("rule.move-up"));
    order.append(Some("Move Down"), Some("rule.move-down"));
    menu.append_section(None, &order);
    let remove = gio::Menu::new();
    remove.append(Some("Delete"), Some("rule.delete"));
    menu.append_section(None, &remove);
    menu
}

/// The row's menu actions, for the rule at `index`.
fn add_actions(actions: &gio::SimpleActionGroup, state: &Rc<State>, index: usize) {
    let entries: [(&str, RowAction); 5] = [
        ("edit", State::edit),
        ("duplicate", State::duplicate),
        ("move-up", |state, index| {
            if index > 0 {
                state.shift(index, index - 1);
            }
        }),
        ("move-down", |state, index| state.shift(index, index + 1)),
        ("delete", State::delete),
    ];
    for (name, run) in entries {
        let action = gio::SimpleAction::new(name, None);
        let state = Rc::downgrade(state);
        action.connect_activate(move |_, _| {
            if let Some(state) = state.upgrade() {
                run(&state, index);
            }
        });
        actions.add_action(&action);
    }
}

/// Pop the row's menu up at `at` (row coordinates), or under the row's
/// start for the keyboard. The popover is dropped when it closes.
fn popup(row: &adw::ActionRow, menu: &gio::Menu, at: Option<(f64, f64)>) {
    let popover = gtk::PopoverMenu::from_model(Some(menu));
    popover.set_parent(row);
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a pointer position inside a row, far inside i32"
    )]
    let (x, y) = at.map_or((24, row.height()), |(x, y)| (x as i32, y as i32));
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x, y, 1, 1)));
    popover.connect_closed(|popover| {
        // GTK runs the item's action as the menu closes, and an action that
        // changes the rules (RUL-04: Duplicate, Move Up, Delete) rebuilds
        // the list and drops this row. Keep the row until the popover has
        // left it: a row finalized with its popover still attached leaves
        // the popover a dangling parent, and unparenting it then crashes.
        let (popover, row) = (popover.clone(), popover.parent());
        glib::idle_add_local_once(move || {
            popover.unparent();
            drop(row);
        });
    });
    popover.popup();
}

/// Dragging a row by its handle carries its index (RUL-04).
fn drag_from(handle: &gtk::Image, row: &adw::ActionRow, index: usize) {
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
            source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&row))), 0, 0);
            row.add_css_class("wye-dragged");
        }
    ));
    source.connect_drag_end(glib::clone!(
        #[weak]
        row,
        move |_, _, _| row.remove_css_class("wye-dragged")
    ));
    handle.add_controller(source);
}

/// The list takes a dragged row: the line shows where it lands, a drop
/// moves it there.
fn accept_drops(list: &gtk::ListBox, state: &Rc<State>) {
    let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
    target.connect_motion(glib::clone!(
        #[weak]
        list,
        #[weak]
        state,
        #[upgrade_or]
        gdk::DragAction::empty(),
        move |_, _, y| {
            state.mark(spot(&list, y));
            gdk::DragAction::MOVE
        }
    ));
    target.connect_leave(glib::clone!(
        #[weak]
        state,
        move |_| state.mark(None)
    ));
    target.connect_drop(glib::clone!(
        #[weak]
        list,
        #[weak]
        state,
        #[upgrade_or]
        false,
        move |_, value, _, y| {
            state.mark(None);
            let (Ok(from), Some((onto, above))) = (value.get::<u32>(), spot(&list, y)) else {
                return false;
            };
            let from = from as usize;
            state.shift(from, drop_index(from, onto, above));
            true
        }
    ));
    list.add_controller(target);
}

/// The row under `y` (list coordinates) and whether `y` is in its upper
/// half.
fn spot(list: &gtk::ListBox, y: f64) -> Option<(usize, bool)> {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a pointer position inside the list, far inside i32"
    )]
    let row = list.row_at_y(y as i32)?;
    let index = usize::try_from(row.index()).ok()?;
    let bounds = row.compute_bounds(list)?;
    let middle = f64::from(bounds.y() + bounds.height() / 2.0);
    Some((index, y < middle))
}
