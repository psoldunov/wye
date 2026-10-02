//! BLK-12 Empty state: the desktop's placeholder message, centred: an
//! `AdwStatusPage` with a large dimmed icon, a title ("No Rules"), a short
//! explanation and an optional action button ("Add Rule…", a pill in the
//! accent colour).
//!
//! API:
//! - [`empty_state`]`(icon, title, description)`: the page (compact, for
//!   use inside a page or a list card).
//! - [`with_action`]`(page, label)`: add the button and return it, to
//!   connect.

/// A compact status page with `icon`, `title` and `description` (markup).
#[must_use]
pub fn empty_state(icon: &str, title: &str, description: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(description)
        .css_classes(["compact"])
        .vexpand(true)
        .build()
}

/// Give `page` an action button labelled `label`, and return it.
pub fn with_action(page: &adw::StatusPage, label: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .css_classes(["pill", "suggested-action"])
        .label(label)
        .halign(gtk::Align::Center)
        .build();
    page.set_child(Some(&button));
    button
}
