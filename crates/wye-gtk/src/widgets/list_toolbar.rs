//! BLK-13 List toolbar: a bar attached to the bottom of a list card, with
//! labelled buttons at the left ("Add Rule…") and a "⋯" menu button at the
//! right that opens and closes its menu.
//!
//! [`ListCard`] is the card: a `boxed-list` `GtkListBox` with the toolbar
//! joined under it, one rounded shape. Put [`ListCard::widget`] in an
//! untitled `AdwPreferencesGroup` (or a group with a title) like any row.
//!
//! API:
//! - [`ListCard::new`]`()`: an empty card; add rows to [`ListCard::list`].
//! - [`ListCard::add_button`]`(label, icon)`: a toolbar button at the left;
//!   returns it to connect.
//! - [`ListCard::set_menu`]`(menu)`: the "⋯" menu at the right (a
//!   `gio::MenuModel` whose actions the page installs);
//!   [`ListCard::menu_button`] is its button.
//! - [`ListCard::set_placeholder`]`(widget)`: shown when the list is empty,
//!   for example an [`super::empty_state`].

use adw::prelude::*;
use gtk::gio;

/// A list with a toolbar under it.
#[derive(Debug, Clone)]
pub struct ListCard {
    widget: gtk::Box,
    list: gtk::ListBox,
    start: gtk::Box,
    menu: gtk::MenuButton,
}

impl Default for ListCard {
    fn default() -> Self {
        Self::new()
    }
}

impl ListCard {
    /// An empty card with an empty toolbar.
    #[must_use]
    pub fn new() -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let start = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .build();
        let menu = gtk::MenuButton::builder()
            .css_classes(["flat"])
            .icon_name("view-more-symbolic")
            .tooltip_text("More")
            .visible(false)
            .build();
        let toolbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .css_classes(["wye-list-toolbar"])
            .build();
        toolbar.append(&start);
        toolbar.append(&menu);
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["wye-list-card"])
            .build();
        widget.append(&list);
        widget.append(&toolbar);
        Self {
            widget,
            list,
            start,
            menu,
        }
    }

    /// The card, to add to a group.
    #[must_use]
    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    /// The list: add rows here.
    #[must_use]
    pub fn list(&self) -> &gtk::ListBox {
        &self.list
    }

    /// A flat button at the left of the toolbar, with `icon` (none when
    /// empty) and `label`.
    pub fn add_button(&self, label: &str, icon: &str) -> gtk::Button {
        let content = adw::ButtonContent::builder().label(label).build();
        if !icon.is_empty() {
            content.set_icon_name(icon);
        }
        let button = gtk::Button::builder()
            .css_classes(["flat"])
            .child(&content)
            .build();
        self.start.append(&button);
        button
    }

    /// The "⋯" menu at the right.
    pub fn set_menu(&self, menu: &impl IsA<gio::MenuModel>) {
        self.menu.set_menu_model(Some(menu));
        self.menu.set_visible(true);
    }

    /// The "⋯" button, to open its menu from code (the self-test).
    #[must_use]
    pub fn menu_button(&self) -> &gtk::MenuButton {
        &self.menu
    }

    /// What the list shows while it has no rows.
    pub fn set_placeholder(&self, placeholder: &impl IsA<gtk::Widget>) {
        self.list.set_placeholder(Some(placeholder));
    }
}
