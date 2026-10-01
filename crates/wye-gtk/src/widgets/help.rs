//! BLK-08 Help button: a flat round contextual-help `GtkMenuButton` (an "i"
//! in a ring, dimmed until hovered, as KDE's) before a row's control,
//! opening a popover that explains the setting. Each is named after its
//! row for screen readers ("Help: Alternative browser").
//! The texts are in 19-help-texts.md and come from
//! [`SettingsStore::help_text`] by ID, so they are audited in one place
//! (`crate::settings::help`).
//!
//! API:
//! - [`help_button`]`(text, topic)`: the button with its popover, named
//!   "Help: `topic`".
//! - [`add_help`]`(store, row, id)`: put the button for help text `id` on
//!   `row`, first among its suffixes (before its control, whenever the
//!   control was added), and keep its text current (some texts name the
//!   configured key). It returns the button, so a row whose help applies
//!   only sometimes (KEY-06) can hide it.

use adw::prelude::*;
use gtk::glib;

use crate::settings::store::SettingsStore;

/// Widest a help popover's text runs, in characters.
const MAX_WIDTH_CHARS: i32 = 44;

/// A round help button about `topic` opening a popover with `text`.
#[must_use]
pub fn help_button(text: &str, topic: &str) -> gtk::MenuButton {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .max_width_chars(MAX_WIDTH_CHARS)
        .xalign(0.0)
        .build();
    let popover = gtk::Popover::builder().child(&label).build();
    // Added, not set: a popover's own `background` class must stay.
    popover.add_css_class("wye-help-popover");
    let button = gtk::MenuButton::builder()
        .css_classes(["flat", "circular", "wye-help-button"])
        .icon_name("wye-help-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text("Help")
        .popover(&popover)
        .build();
    name_after(&button, topic);
    button
}

/// Name `button` "Help: `topic`" (just "Help" without a topic). `topic`
/// may be a row title in markup: the name is its text ("&amp;" reads "&").
fn name_after(button: &gtk::MenuButton, topic: &str) {
    let name = if topic.is_empty() {
        "Help".to_owned()
    } else {
        format!("Help: {}", plain(topic))
    };
    button.update_property(&[gtk::accessible::Property::Label(&name)]);
}

/// Add the help button for text `id` to `row`, kept current with the store.
pub fn add_help(
    store: &SettingsStore,
    row: &impl IsA<adw::ActionRow>,
    id: &'static str,
) -> gtk::MenuButton {
    let row = row.upcast_ref::<adw::ActionRow>();
    let button = help_button(&store.help_text(id), &row.title());
    // The row's title can change after (a page fills it in later).
    row.connect_title_notify(glib::clone!(
        #[weak]
        button,
        move |row| name_after(&button, &row.title())
    ));
    row.add_suffix(&button);
    // First among the suffixes, so it sits before the row's control even
    // when the control came first (an `AdwSwitchRow`'s own switch).
    if let Some(suffixes) = button.parent().and_downcast::<gtk::Box>() {
        suffixes.reorder_child_after(&button, None::<&gtk::Widget>);
    }
    store.connect_changed_while(
        &button,
        glib::clone!(
            #[weak]
            button,
            move |store| {
                let text = store.help_text(id);
                let label = button
                    .popover()
                    .and_then(|popover| popover.child())
                    .and_downcast::<gtk::Label>();
                if let Some(label) = label
                    && label.label() != text
                {
                    label.set_label(&text);
                }
            }
        ),
    );
    button
}

/// The text of `markup`; `markup` itself when it is not valid markup.
fn plain(markup: &str) -> String {
    gtk::pango::parse_markup(markup, '\0')
        .map_or_else(|_| markup.to_owned(), |(_, text, _)| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn blk_08_a_help_button_is_named_by_its_rows_text() {
        assert_eq!(plain("Tools &amp; <b>more</b>"), "Tools & more");
        assert_eq!(plain("Broken <b"), "Broken <b");
    }
}
