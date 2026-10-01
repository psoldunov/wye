//! Icons in a `GtkPopoverMenu`'s items, for the menus that list targets (the
//! tray menu's browsers, TRAY-08; the picker's Open In, PICK-28), as the KDE
//! menus show them. GTK draws a menu item's icon only when the item has no
//! label, so the kit's [`target_icon`](super::icon::target_icon) (BLK-04,
//! with its profile badge) goes into the item's own button, before its
//! label. The items stay GTK's: focus and arrow keys, the radio dot, the
//! shortcut on the right, submenus. On a menu page where any item has an
//! icon, the others get an empty square of the same size, so every label
//! lines up as GTK lines up its radio dots.
//!
//! Items are found by their label, in order, after `set_menu_model` (GTK
//! builds them, nested submenus included, then). An item GTK draws in a way
//! this does not recognise keeps its plain label.

use std::collections::{HashMap, VecDeque};

use gtk::prelude::*;
use serde_json::Value;

use super::icon;

/// The icon's size: GTK's menu icons are 16 px.
const SIZE: i32 = 16;
/// The gap between the icon and the label.
const GAP: i32 = 8;
/// The CSS name of the button GTK draws a menu item with.
const ITEM: &str = "modelbutton";
/// libadwaita's start padding of a menu item (`popover.menu modelbutton`)
/// and of a section heading (`popover.menu label.title`).
const ITEM_PADDING: i32 = 12;
const HEADING_PADDING: i32 = 32;

/// The icon of the item labelled `label`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemIcon {
    pub label: String,
    /// Icon theme name or file path.
    pub source: String,
    /// The service's badge (`{"initial", "color"}` or `{"image"}`), if any.
    pub badge: Option<Value>,
}

/// A menu item and the icon it gets, if any.
type Entry<'a> = (gtk::Widget, Option<&'a ItemIcon>);

/// Put `icons` into the items of `popover` with their labels; two items of
/// one label take the icons in order.
pub fn add(popover: &gtk::PopoverMenu, icons: &[ItemIcon]) {
    let mut wanted: HashMap<&str, VecDeque<&ItemIcon>> = HashMap::new();
    for icon in icons.iter().filter(|icon| !icon.source.is_empty()) {
        wanted
            .entry(icon.label.as_str())
            .or_default()
            .push_back(icon);
    }
    if wanted.is_empty() {
        return;
    }
    let mut pages: Vec<(Option<gtk::Widget>, Vec<Entry>)> = Vec::new();
    for item in items(popover.upcast_ref()) {
        let text = item.property::<Option<String>>("text").unwrap_or_default();
        let icon = wanted.get_mut(text.as_str()).and_then(VecDeque::pop_front);
        let page = page(&item);
        match pages.iter_mut().find(|(known, _)| *known == page) {
            Some((_, entries)) => entries.push((item, icon)),
            None => pages.push((page, vec![(item, icon)])),
        }
    }
    for (page, entries) in pages {
        if entries.iter().all(|(_, icon)| icon.is_none()) {
            continue;
        }
        if let (Some(page), Some((first, _))) = (page, entries.first()) {
            align_headings(&page, first);
        }
        for (item, icon) in entries {
            let widget = icon.map_or_else(
                || gtk::Box::builder().width_request(SIZE).build().upcast(),
                |icon| icon::target_icon(&icon.source, icon.badge.as_ref(), SIZE),
            );
            insert(&item, &widget);
        }
    }
}

/// Start the section headings of `page` where the labels of its items, like
/// `item`, now do: after the item's padding, its radio column and the icon.
/// GTK adds the headings in an idle of its own at this priority, scheduled
/// earlier, so this one runs after it and before the menu is drawn.
fn align_headings(page: &gtk::Widget, item: &gtk::Widget) {
    let indicators = item.first_child().map_or(0, |column| {
        column.measure(gtk::Orientation::Horizontal, -1).1
    });
    let indent = (ITEM_PADDING + indicators + SIZE + GAP - HEADING_PADDING).max(0);
    let page = page.downgrade();
    gtk::glib::idle_add_local_full(gtk::glib::Priority::DEFAULT, move || {
        for heading in page.upgrade().as_ref().map(headings).unwrap_or_default() {
            heading.set_margin_start(indent);
        }
        gtk::glib::ControlFlow::Break
    });
}

/// The section headings on menu page `page` (GTK draws one as a label
/// styled `title` and `separator`), not those of nested submenus.
fn headings(page: &gtk::Widget) -> Vec<gtk::Widget> {
    fn walk(widget: &gtk::Widget, found: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(part) = child {
            if part.is::<gtk::Label>()
                && part.has_css_class("title")
                && part.has_css_class("separator")
            {
                found.push(part.clone());
            }
            walk(&part, found);
            child = part.next_sibling();
        }
    }
    let mut found = Vec::new();
    walk(page, &mut found);
    found.retain(|heading| self::page(heading).as_ref() == Some(page));
    found
}

/// The item of `popover` labelled `label`, for the self-test to open a
/// submenu with.
pub fn item(popover: &gtk::PopoverMenu, label: &str) -> Option<gtk::Widget> {
    items(popover.upcast_ref())
        .into_iter()
        .find(|item| item.property::<Option<String>>("text").as_deref() == Some(label))
}

/// Every menu item under `root`, nested submenus included, in tree order;
/// not a sliding submenu's title (its "back" button).
fn items(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.css_name() == ITEM && !widget.has_css_class("title") {
            found.push(widget.clone());
        }
        found.extend(items(&widget));
        child = widget.next_sibling();
    }
    found
}

/// The menu page `item` is on: the child of the nearest stack (a popover
/// menu shows its submenus, or only its main page, in a `GtkStack`).
fn page(item: &gtk::Widget) -> Option<gtk::Widget> {
    let mut widget = item.clone();
    loop {
        let parent = widget.parent()?;
        if parent.is::<gtk::Stack>() {
            return Some(widget);
        }
        widget = parent;
    }
}

/// `widget` before the label of `item`. GTK removes only its own parts of
/// an item when it goes, so `widget` leaves with it.
fn insert(item: &gtk::Widget, widget: &gtk::Widget) {
    let mut child = item.first_child();
    while let Some(part) = child {
        if part.is::<gtk::Label>() {
            widget.set_margin_end(GAP);
            widget.set_valign(gtk::Align::Center);
            widget.set_can_target(false);
            widget.insert_before(item, Some(&part));
            let widget = widget.clone();
            item.connect_destroy(move |_| widget.unparent());
            return;
        }
        child = part.next_sibling();
    }
}
