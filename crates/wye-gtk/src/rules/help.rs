//! The rules help (RUL-19, 19-help-texts.md "Rules help"): the rule
//! editor's "?" opens a dialog, not a popover, with seven sections. Each
//! section is a titled group with its text as the description; the URL
//! matchers section lists every kind as a row: its name, its example
//! pattern in the fixed-width font, and what that pattern matches and does
//! not.
//!
//! KDE counterpart: crates/wye-ui/qml/rules/RulesHelpDialog.qml.

use adw::prelude::*;
use gtk::glib;

use super::matcher_row;
use crate::widgets::group;

/// The dialog's size in logical pixels.
const SIZE: (i32, i32) = (560, 640);

/// The sections in order: heading and text (markup).
const SECTIONS: [(&str, &str); 7] = [
    (
        "How rules work",
        "Rules are checked from the top of the list down; the first rule that matches decides where the link opens. A rule can check the link, the app it was clicked in, and the keys held while clicking.",
    ),
    (
        "URL matchers",
        "Wye removes <tt>https://</tt> and a leading <tt>www.</tt> before matching.",
    ),
    (
        "Source apps",
        "The rule matches only links opened from one of these apps. Some apps (for example sandboxed Flatpak apps) cannot always be identified; rules with source apps then do not match.",
    ),
    (
        "Held keys",
        "The rule matches only while exactly these modifier keys are held.",
    ),
    (
        "Before or after built-in rules",
        "Built-in rules are the mappings on the Apps page. “Before” lets a rule override them.",
    ),
    (
        "Transform URL",
        "A script that rewrites the link when this rule matches. See the script editor's Reference.",
    ),
    (
        "Testing",
        "Use <b>Test Rules…</b> in the Rules page menu to see which rule a link hits.",
    ),
];

/// For each URL matcher kind, in the editor's order
/// ([`matcher_row::KINDS`], which has the names and example patterns): a
/// link the example matches and one it does not (19-help-texts.md's table).
const EXAMPLES: [(&str, &str); 5] = [
    ("github.com/x, gist.github.com/y", "notgithub.com"),
    (
        "docs.google.com/spreadsheets/d/1",
        "docs.google.com/document/d/1",
    ),
    ("github.com/a/b/pull/7", "github.com/a/b/issues/7"),
    ("team.atlassian.net/browse/ABC-1", "atlassian.net/wiki"),
    ("meet.google.com/abc-defg-hij", "meet.google.com/landing"),
];

/// Show the rules help over the window (or sheet) `parent` is in.
pub fn open(parent: &impl IsA<gtk::Widget>) -> adw::Dialog {
    let page = adw::PreferencesPage::new();
    for (index, (heading, text)) in SECTIONS.iter().enumerate() {
        let section = group::group_with_description(heading, text);
        if index == 1 {
            for ((_, kind, pattern), (matches, misses)) in matcher_row::KINDS.iter().zip(EXAMPLES) {
                section.add(&kind_row(kind, pattern, matches, misses));
            }
        }
        page.add(&section);
    }
    let header = adw::HeaderBar::new();
    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&header);
    let dialog = adw::Dialog::builder()
        .title("How Rules Work")
        .child(&view)
        .content_width(SIZE.0)
        .content_height(SIZE.1)
        .build();
    dialog.add_css_class("wye-rules-help");
    dialog.present(Some(parent));
    dialog
}

/// One kind: its name, then its example pattern in the fixed-width font
/// and what that pattern matches and does not.
fn kind_row(kind: &str, pattern: &str, matches: &str, misses: &str) -> adw::ActionRow {
    let subtitle = format!(
        "<tt>{}</tt>\nMatches {}\nDoes not match {}",
        glib::markup_escape_text(pattern),
        glib::markup_escape_text(matches),
        glib::markup_escape_text(misses)
    );
    adw::ActionRow::builder()
        .title(kind)
        .subtitle(subtitle)
        .subtitle_selectable(true)
        .activatable(false)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rul_19_the_help_has_seven_sections_and_every_kind() {
        let headings: Vec<&str> = SECTIONS.iter().map(|(heading, _)| *heading).collect();
        assert_eq!(headings.len(), 7);
        assert_eq!(headings[1], "URL matchers");
        assert_eq!(
            EXAMPLES.len(),
            matcher_row::KINDS.len(),
            "an example per kind"
        );
        for (_, text) in SECTIONS {
            assert!(gtk::pango::parse_markup(text, '\0').is_ok(), "{text}");
        }
    }
}
