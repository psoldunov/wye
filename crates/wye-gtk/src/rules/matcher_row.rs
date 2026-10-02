//! One URL matcher of the rule editor (RUL-14): a kind popup, the **Match**
//! entry whose placeholder is an example for the kind, and a remove button;
//! an invalid pattern shows its error under the entry, in the error colour.
//! A row of the section's `boxed-list`, not activatable: its controls take
//! the focus and the clicks.
//!
//! KDE counterpart: crates/wye-ui/qml/rules/RuleMatcherRow.qml.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

/// The kinds, as stored, with their labels and an example pattern each
/// (19-help-texts.md, "URL matchers").
pub const KINDS: [(&str, &str, &str); 5] = [
    ("domain", "Domain", "github.com"),
    ("prefix", "Starts with", "docs.google.com/spreadsheets"),
    ("contains", "Contains", "/pull/"),
    ("wildcard", "Wildcard", "*.atlassian.net/browse/*"),
    (
        "regex",
        "Regular expression",
        "^meet\\.google\\.com/[a-z]{3}-",
    ),
];

/// The kind popup's arrow, spacing and padding around its label, in
/// logical pixels.
const KIND_BOX_CHROME: i32 = 44;

/// A matcher row. Clones share it.
#[derive(Debug, Clone)]
pub struct MatcherRow {
    row: gtk::ListBoxRow,
    kind: gtk::DropDown,
    entry: gtk::Entry,
    remove: gtk::Button,
    error: gtk::Label,
    error_line: gtk::Box,
}

impl MatcherRow {
    /// A row showing `kind` and `pattern`.
    #[must_use]
    pub fn new(kind: &str, pattern: &str) -> Self {
        let labels: Vec<&str> = KINDS.iter().map(|(_, label, _)| *label).collect();
        let kind_box = gtk::DropDown::from_strings(&labels);
        kind_box.set_valign(gtk::Align::Center);
        kind_box.update_property(&[gtk::accessible::Property::Label("Kind")]);
        kind_box.set_selected(kind_index(kind));
        let entry = gtk::Entry::builder()
            .text(pattern)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .placeholder_text(example(kind))
            .input_purpose(gtk::InputPurpose::Url)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label("Match")]);
        let remove = gtk::Button::builder()
            .css_classes(["flat", "circular"])
            .icon_name("list-remove-symbolic")
            .tooltip_text("Remove Matcher")
            .valign(gtk::Align::Center)
            .build();
        let controls = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        controls.append(&kind_box);
        controls.append(&entry);
        controls.append(&remove);
        let (error, error_line) = error_line();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["wye-matcher-row"])
            .build();
        content.append(&controls);
        content.append(&error_line);
        let row = gtk::ListBoxRow::builder()
            .activatable(false)
            .selectable(false)
            .focusable(false)
            .child(&content)
            .build();
        row.update_property(&[gtk::accessible::Property::Label("URL matcher")]);
        let this = Self {
            row,
            kind: kind_box,
            entry,
            remove,
            error,
            error_line,
        };
        this.size_kind_box();
        this
    }

    /// The row, to add to the section.
    #[must_use]
    pub fn row(&self) -> &gtk::ListBoxRow {
        &self.row
    }

    /// Run `edited(kind, pattern)` after the user changes the kind or the
    /// pattern.
    pub fn connect_edited(&self, edited: impl Fn(&'static str, String) + 'static) {
        let edited = Rc::new(edited);
        self.kind.connect_selected_notify(glib::clone!(
            #[strong]
            edited,
            #[weak(rename_to = entry)]
            self.entry,
            move |kind| {
                let index = usize::try_from(kind.selected()).unwrap_or_default();
                let (value, _, example) = KINDS.get(index).copied().unwrap_or(KINDS[0]);
                entry.set_placeholder_text(Some(example));
                edited(value, entry.text().to_string());
            }
        ));
        self.entry.connect_changed(glib::clone!(
            #[weak(rename_to = kind)]
            self.kind,
            move |entry| {
                let index = usize::try_from(kind.selected()).unwrap_or_default();
                let value = KINDS.get(index).map_or("domain", |(value, _, _)| *value);
                edited(value, entry.text().to_string());
            }
        ));
    }

    /// Run `remove()` when the remove button is pressed.
    pub fn connect_remove(&self, remove: impl Fn() + 'static) {
        self.remove.connect_clicked(move |_| remove());
    }

    /// Show `message` under the entry; empty hides it. An empty pattern says
    /// nothing yet: Save stays disabled and the editor says why (RUL-18).
    pub fn set_error(&self, message: &str) {
        let shown = !message.is_empty() && !self.entry.text().is_empty();
        self.error.set_label(message);
        self.error_line.set_visible(shown);
        if shown {
            self.entry.add_css_class("error");
        } else {
            self.entry.remove_css_class("error");
        }
        self.entry
            .update_property(&[gtk::accessible::Property::Description(if shown {
                message
            } else {
                ""
            })]);
    }

    /// Put the cursor in the entry ("+" adds a row and focuses it).
    pub fn focus_entry(&self) {
        self.entry.grab_focus();
    }

    /// Every row's kind popup as wide as the widest kind, so the entries
    /// start at the same place.
    fn size_kind_box(&self) {
        let widest = KINDS
            .iter()
            .map(|(_, label, _)| {
                let probe = gtk::Label::new(Some(label));
                probe.measure(gtk::Orientation::Horizontal, -1).1
            })
            .max()
            .unwrap_or_default();
        self.kind.set_size_request(widest + KIND_BOX_CHROME, -1);
    }
}

/// The position of `kind` in [`KINDS`]; Domain when unknown.
fn kind_index(kind: &str) -> u32 {
    KINDS
        .iter()
        .position(|(value, _, _)| *value == kind)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// The example pattern for `kind`.
fn example(kind: &str) -> &'static str {
    KINDS
        .iter()
        .find(|(value, _, _)| *value == kind)
        .map_or(KINDS[0].2, |(_, _, example)| *example)
}

/// The line under a matcher that says what is wrong with it (RUL-18): an
/// error glyph and the text, hidden while there is nothing to say.
fn error_line() -> (gtk::Label, gtk::Box) {
    let error = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .hexpand(true)
        .css_classes(["caption", "error"])
        .build();
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .visible(false)
        .css_classes(["wye-matcher-error"])
        .build();
    let glyph = gtk::Image::builder()
        .icon_name("dialog-error-symbolic")
        .pixel_size(14)
        .valign(gtk::Align::Start)
        .css_classes(["error"])
        .build();
    line.append(&glyph);
    line.append(&error);
    (error, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rul_14_kinds_are_the_stored_names_with_examples() {
        let stored: Vec<&str> = KINDS.iter().map(|(kind, _, _)| *kind).collect();
        assert_eq!(
            stored,
            ["domain", "prefix", "contains", "wildcard", "regex"]
        );
        for kind in stored {
            let matcher: wye_core::matcher::UrlMatcher =
                serde_json::from_value(serde_json::json!({"kind": kind, "pattern": example(kind)}))
                    .expect("a stored kind");
            assert!(matcher.compile().is_ok(), "{kind}'s example is valid");
        }
        assert_eq!(kind_index("regex"), 4);
        assert_eq!(kind_index("nope"), 0);
        assert_eq!(example("nope"), "github.com");
    }
}
