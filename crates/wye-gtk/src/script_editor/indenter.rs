//! SCR-02 auto-indent: `GtkSourceView`'s indenter with wye-ui's rule
//! ([`editing::newline_with_indent`]): Return keeps the line's indentation and
//! adds one level after an opening bracket. Return between a pair
//! (`{|}`) also puts the closing bracket on a line of its own, back at the
//! outer level, with the cursor on the indented line between.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use sourceview5::subclass::prelude::*;

use super::editing;

/// Closing brackets that go to their own line after Return between a pair.
const CLOSERS: [char; 3] = [')', ']', '}'];

mod imp {
    use super::{IndenterImpl, ObjectImpl, ObjectSubclass, glib};

    #[derive(Debug, Default)]
    pub struct Indenter;

    #[glib::object_subclass]
    impl ObjectSubclass for Indenter {
        const NAME: &'static str = "WyeScriptIndenter";
        type Type = super::Indenter;
        type Interfaces = (sourceview5::Indenter,);
    }

    impl ObjectImpl for Indenter {}

    impl IndenterImpl for Indenter {
        fn is_trigger(
            &self,
            _view: &sourceview5::View,
            _location: &gtk::TextIter,
            state: gtk::gdk::ModifierType,
            keyval: gtk::gdk::Key,
        ) -> bool {
            super::is_trigger(state, keyval)
        }

        fn indent(&self, _view: &sourceview5::View, iter: &mut gtk::TextIter) {
            super::indent(iter);
        }
    }
}

glib::wrapper! {
    /// The code area's indenter.
    pub struct Indenter(ObjectSubclass<imp::Indenter>) @implements sourceview5::Indenter;
}

impl Indenter {
    #[must_use]
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for Indenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Return and Enter without Control or Alt.
fn is_trigger(state: gtk::gdk::ModifierType, keyval: gtk::gdk::Key) -> bool {
    let plain =
        !state.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK);
    plain
        && matches!(
            keyval,
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::ISO_Enter
        )
}

/// Indent the line that starts at `iter`, just after the newline was
/// inserted; `iter` ends where the cursor goes.
fn indent(iter: &mut gtk::TextIter) {
    let buffer = iter.buffer();
    let mut previous = *iter;
    if !previous.backward_line() {
        return;
    }
    let line = buffer.text(&previous, iter, false);
    let line = line.trim_end_matches(['\n', '\r']);
    let wanted = indentation(line);
    let outer: String = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let closes = CLOSERS.contains(&iter.char());
    if closes && wanted.len() > outer.len() {
        buffer.insert(iter, &format!("{wanted}\n{outer}"));
        let back = i32::try_from(outer.chars().count() + 1).unwrap_or(0);
        iter.backward_chars(back);
    } else {
        buffer.insert(iter, &wanted);
    }
}

/// The indentation the line after `line` starts with.
fn indentation(line: &str) -> String {
    let end = line.encode_utf16().count();
    let text = editing::newline_with_indent(line, end);
    text.strip_prefix('\n').unwrap_or(&text).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scr_02_the_next_line_keeps_and_deepens_the_indent() {
        assert_eq!(indentation("  return url;"), "  ");
        assert_eq!(indentation("  if (x) {"), "    ");
        assert_eq!(indentation(""), "");
    }

    #[test]
    fn scr_02_only_a_plain_return_indents() {
        let none = gtk::gdk::ModifierType::empty();
        assert!(is_trigger(none, gtk::gdk::Key::Return));
        assert!(is_trigger(
            gtk::gdk::ModifierType::SHIFT_MASK,
            gtk::gdk::Key::KP_Enter
        ));
        assert!(!is_trigger(
            gtk::gdk::ModifierType::CONTROL_MASK,
            gtk::gdk::Key::Return
        ));
        assert!(!is_trigger(none, gtk::gdk::Key::Tab));
    }
}
