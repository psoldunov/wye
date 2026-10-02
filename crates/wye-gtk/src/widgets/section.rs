//! BLK-14 Section with add button: a bold section title with a small "+"
//! button beside it (the group's header suffix), a dimmed subtitle, then a
//! list card that shows a placeholder ("No Matchers") while it is empty.
//!
//! API:
//! - [`AddSection::new`]`(title, subtitle, placeholder)`: the section; add
//!   rows with [`AddSection::add`], remove them with [`AddSection::remove`].
//! - [`AddSection::add_button`]: the "+", to connect.
//! - [`AddSection::group`]: the `AdwPreferencesGroup` to add to the page.
//! - [`clear_rows`]`(list)`: empty any list and keep its placeholder.

use adw::prelude::*;

/// A titled list with a "+" button.
#[derive(Debug, Clone)]
pub struct AddSection {
    group: adw::PreferencesGroup,
    list: gtk::ListBox,
    add: gtk::Button,
}

impl AddSection {
    /// A section titled `title` with `subtitle` (markup) and `placeholder`
    /// for the empty list.
    #[must_use]
    pub fn new(title: &str, subtitle: &str, placeholder: &str) -> Self {
        let add = gtk::Button::builder()
            .css_classes(["flat", "circular"])
            .icon_name("list-add-symbolic")
            .tooltip_text(format!("Add to {title}"))
            .valign(gtk::Align::Center)
            .build();
        let group = adw::PreferencesGroup::builder()
            .title(title)
            .header_suffix(&add)
            .build();
        group.set_description(Some(subtitle).filter(|text| !text.is_empty()));
        let empty = gtk::Label::builder()
            .label(placeholder)
            .margin_top(14)
            .margin_bottom(14)
            .css_classes(["dimmed"])
            .build();
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list.set_placeholder(Some(&empty));
        group.add(&list);
        Self { group, list, add }
    }

    /// The group, to add to the page.
    #[must_use]
    pub fn group(&self) -> &adw::PreferencesGroup {
        &self.group
    }

    /// The "+" button.
    #[must_use]
    pub fn add_button(&self) -> &gtk::Button {
        &self.add
    }

    /// Append `row` to the list.
    pub fn add(&self, row: &impl IsA<gtk::Widget>) {
        self.list.append(row);
    }

    /// Remove `row` from the list.
    pub fn remove(&self, row: &impl IsA<gtk::Widget>) {
        self.list.remove(row);
    }

    /// Remove every row (the placeholder shows).
    pub fn clear(&self) {
        clear_rows(&self.list);
    }
}

/// Remove every row of `list` but keep its placeholder:
/// `gtk_list_box_remove_all` removes the placeholder too, so an emptied
/// list would show nothing.
pub fn clear_rows(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}
