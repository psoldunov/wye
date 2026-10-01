//! BLK-09 Callout: an inline information card in the page's column, with an
//! information icon, an optional bold title ("Please Read"), text that may
//! use bold and inline code (`<b>`, `<tt>`) and links (sent through Wye),
//! and a close button at the end. Closing hides it for good: its ID goes
//! into the service's UI state (`UpdateUiState`, `uiState.dismissedCallouts`)
//! through the store, so it stays hidden across runs.
//!
//! It lives in its own untitled `AdwPreferencesGroup`, so it lines up with
//! the groups around it; it stays hidden until the store has loaded, so a
//! dismissed callout never flashes up.
//!
//! API:
//! - [`Callout::new`]`(store, id, title, markup)`; add [`Callout::group`] to
//!   the page.

use adw::prelude::*;
use gtk::glib;

use super::links;
use crate::settings::store::SettingsStore;

/// A dismissible information card.
#[derive(Debug, Clone)]
pub struct Callout {
    group: adw::PreferencesGroup,
}

impl Callout {
    /// The callout `id` with `title` (none when empty) and `markup`.
    #[must_use]
    pub fn new(store: &SettingsStore, id: &'static str, title: &str, markup: &str) -> Self {
        let icon = gtk::Image::builder()
            .icon_name("dialog-information-symbolic")
            .pixel_size(16)
            .valign(gtk::Align::Start)
            .margin_top(2)
            .css_classes(["wye-callout-icon"])
            .build();
        let label = text(title, markup);
        links::route_links(
            &label,
            glib::clone!(
                #[weak]
                store,
                move |url| store.open_link(url)
            ),
        );
        let close = gtk::Button::builder()
            .css_classes(["flat", "circular"])
            .icon_name("window-close-symbolic")
            .valign(gtk::Align::Start)
            .tooltip_text("Dismiss")
            .build();
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["wye-callout"])
            .build();
        card.append(&icon);
        card.append(&label);
        card.append(&close);
        card.update_property(&[gtk::accessible::Property::Label(&format!(
            "Information: {}",
            label.text()
        ))]);
        let group = adw::PreferencesGroup::new();
        group.add(&card);
        close.connect_clicked(glib::clone!(
            #[weak]
            store,
            #[weak]
            group,
            move |_| {
                group.set_visible(false);
                store.dismiss_callout(id);
            }
        ));
        let show = glib::clone!(
            #[weak]
            group,
            move |store: &SettingsStore| {
                group.set_visible(store.loaded() && !store.callout_dismissed(id));
            }
        );
        show(store);
        store.connect_changed_while(&group, show);
        Self { group }
    }

    /// The group holding the card, to add to the page.
    #[must_use]
    pub fn group(&self) -> &adw::PreferencesGroup {
        &self.group
    }
}

/// The callout's text: `title` in bold (when not empty) over `markup`.
fn text(title: &str, markup: &str) -> gtk::Label {
    let markup = keep_last_words(markup);
    let text = if title.is_empty() {
        markup
    } else {
        format!("<b>{}</b>\n{markup}", glib::markup_escape_text(title))
    };
    gtk::Label::builder()
        .label(text)
        .use_markup(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["wye-callout-text"])
        .build()
}

/// `markup` with the last two words of each paragraph joined by a
/// no-break space, so a wrapped paragraph never ends on one orphaned word
/// ("menu."). Spaces inside tags are left alone.
fn keep_last_words(markup: &str) -> String {
    markup
        .split('\n')
        .map(|line| {
            let mut in_tag = false;
            let mut last = None;
            for (index, character) in line.char_indices() {
                match character {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    ' ' if !in_tag => last = Some(index),
                    _ => {}
                }
            }
            let parts = last.and_then(|index| Some((line.get(..index)?, line.get(index + 1..)?)));
            match parts {
                Some((head, tail)) => format!("{head}\u{a0}{tail}"),
                None => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::keep_last_words;

    #[test]
    fn the_last_two_words_of_a_paragraph_stay_together() {
        assert_eq!(
            keep_last_words("choose it in the Wye menu."),
            "choose it in the Wye\u{a0}menu."
        );
        assert_eq!(
            keep_last_words("one <a href=\"x\">two</a>\n\nthree"),
            "one\u{a0}<a href=\"x\">two</a>\n\nthree"
        );
    }
}
