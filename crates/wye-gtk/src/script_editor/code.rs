//! The code area (SCR-02, SCR-05): a `GtkSourceView` with JavaScript
//! highlighting, line numbers, the current line highlighted, bracket
//! matching, auto-indent ([`Indenter`]), two-space indents as on KDE
//! ([`editing::indent`]), the document's monospace font and its own
//! undo/redo; the error line tinted and underlined. The colours follow the
//! style manager's light or dark style live, in the code area and in the
//! Reference's samples alike ([`follow_style`]).
//!
//! It sits in a rounded, hairline-framed card (`.wye-script-code`).

use adw::prelude::*;
use gtk::{glib, pango};
use sourceview5::prelude::*;

use super::editing;
use super::indenter::Indenter;
use super::palette;

/// `GtkSourceView`'s JavaScript language.
const LANGUAGE: &str = "js";
/// Wye's schemes, light and dark: `GtkSourceView`'s Adwaita ones with the
/// built-in methods and properties coloured too (`data/styles/`).
const LIGHT_SCHEME: &str = "wye-script";
const DARK_SCHEME: &str = "wye-script-dark";
/// Their parents, should the `GResource` lack them.
const FALLBACK_SCHEMES: (&str, &str) = ("Adwaita", "Adwaita-dark");
/// Where the `GResource` keeps Wye's schemes.
const SCHEMES_PATH: &str = "resource:///dev/soldunov/wye/styles";
/// SCR-05: the tag on the error line.
const ERROR_TAG: &str = "wye-error-line";
/// How strongly the error line is tinted, in the light and the dark style.
const ERROR_TINT: (f32, f32) = (0.14, 0.24);
/// Room around the text inside the card.
const TEXT_MARGIN: i32 = 10;

/// The code area.
#[derive(Debug, Clone)]
pub struct Code {
    /// The card to place.
    pub widget: gtk::ScrolledWindow,
    pub view: sourceview5::View,
    pub buffer: sourceview5::Buffer,
}

impl Code {
    #[must_use]
    pub fn new() -> Self {
        let buffer = buffer();
        buffer.set_highlight_matching_brackets(true);
        let width = u32::try_from(editing::indent().len()).unwrap_or(2);
        let view = sourceview5::View::builder()
            .buffer(&buffer)
            .monospace(true)
            .show_line_numbers(true)
            .highlight_current_line(true)
            .auto_indent(true)
            .indenter(&Indenter::new())
            .indent_width(i32::try_from(width).unwrap_or(2))
            .tab_width(width)
            .insert_spaces_instead_of_tabs(true)
            .smart_backspace(true)
            .smart_home_end(sourceview5::SmartHomeEndType::Before)
            .wrap_mode(gtk::WrapMode::None)
            .top_margin(TEXT_MARGIN)
            .bottom_margin(TEXT_MARGIN)
            .left_margin(TEXT_MARGIN / 2)
            // None, so the error line's tint reaches the frame.
            .right_margin(0)
            .build();
        view.upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Label("Script")]);
        buffer.create_tag(Some(ERROR_TAG), &[("underline", &pango::Underline::Error)]);
        let widget = gtk::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .hexpand(true)
            .build();
        widget.add_css_class("wye-script-code");
        widget.set_overflow(gtk::Overflow::Hidden);
        let code = Self {
            widget,
            view,
            buffer,
        };
        tint(&code.buffer);
        let style = adw::StyleManager::default();
        style.connect_dark_notify(glib::clone!(
            #[weak(rename_to = buffer)]
            code.buffer,
            move |_| tint(&buffer)
        ));
        code
    }

    /// Replace the text without an undo step (a load): the cursor goes to
    /// the start and the view to the top.
    pub fn load(&self, text: &str) {
        self.buffer.begin_irreversible_action();
        self.buffer.set_text(text);
        self.buffer.end_irreversible_action();
        self.buffer.place_cursor(&self.buffer.start_iter());
        self.buffer.set_modified(false);
        self.widget.vadjustment().set_value(0.0);
    }

    /// Replace the text as the user would, as one undo step (the self-test's
    /// unsaved changes).
    pub fn type_text(&self, text: &str) {
        let buffer = &self.buffer;
        buffer.begin_user_action();
        let (mut start, mut end) = buffer.bounds();
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, text);
        buffer.end_user_action();
    }

    /// The whole text.
    #[must_use]
    pub fn text(&self) -> String {
        let (start, end) = self.buffer.bounds();
        self.buffer.text(&start, &end, true).into()
    }

    /// SCR-05: mark `line` (from 1); 0 clears the mark.
    pub fn mark_error(&self, line: u32) {
        let (start, end) = self.buffer.bounds();
        self.buffer.remove_tag_by_name(ERROR_TAG, &start, &end);
        let Some(index) = line
            .checked_sub(1)
            .and_then(|index| i32::try_from(index).ok())
        else {
            return;
        };
        let Some(from) = self.buffer.iter_at_line(index) else {
            return;
        };
        let mut to = from;
        if !to.ends_line() {
            to.forward_to_line_end();
        }
        // The newline too, so the tint spans an empty line.
        to.forward_char();
        self.buffer.apply_tag_by_name(ERROR_TAG, &from, &to);
    }
}

impl Default for Code {
    fn default() -> Self {
        Self::new()
    }
}

/// A buffer of JavaScript in the style now on screen, following it.
#[must_use]
pub fn buffer() -> sourceview5::Buffer {
    let buffer = sourceview5::Buffer::new(None);
    if let Some(language) = sourceview5::LanguageManager::default().language(LANGUAGE) {
        buffer.set_language(Some(&language));
    } else {
        tracing::warn!("GtkSourceView has no JavaScript language; the script is not highlighted");
    }
    follow_style(&buffer);
    buffer
}

/// Give `buffer` the Adwaita scheme of the style on screen, now and whenever
/// the style changes.
fn follow_style(buffer: &sourceview5::Buffer) {
    let style = adw::StyleManager::default();
    apply_scheme(buffer, style.is_dark());
    style.connect_dark_notify(glib::clone!(
        #[weak]
        buffer,
        move |style| apply_scheme(&buffer, style.is_dark())
    ));
}

fn apply_scheme(buffer: &sourceview5::Buffer, dark: bool) {
    let manager = sourceview5::StyleSchemeManager::default();
    if !manager
        .search_path()
        .iter()
        .any(|path| path == SCHEMES_PATH)
    {
        manager.append_search_path(SCHEMES_PATH);
    }
    let (id, fallback) = if dark {
        (DARK_SCHEME, FALLBACK_SCHEMES.1)
    } else {
        (LIGHT_SCHEME, FALLBACK_SCHEMES.0)
    };
    if let Some(scheme) = manager.scheme(id).or_else(|| manager.scheme(fallback)) {
        buffer.set_style_scheme(Some(&scheme));
    } else {
        tracing::warn!(%id, "GtkSourceView has no such style scheme");
    }
}

/// The error tag in the error colour of the style on screen.
fn tint(buffer: &sourceview5::Buffer) {
    let Some(tag) = buffer.tag_table().lookup(ERROR_TAG) else {
        return;
    };
    let colours = palette::current();
    tag.set_underline_rgba(Some(&palette::rgba(colours.error, 1.0)));
    let tint = if adw::StyleManager::default().is_dark() {
        ERROR_TINT.1
    } else {
        ERROR_TINT.0
    };
    tag.set_paragraph_background_rgba(Some(&palette::rgba(colours.error_bg, tint)));
}
