//! The Test group (SCR-04): **Link** with the **Source app** popup at its
//! end, **Result**, in a boxed list, and **Log** when the script called
//! `console.log`. Kept compact, so the code area gets the window's height.
//!
//! The result reads as a property row: "Result" small above the value. The
//! value is the returned link with the parts the script changed marked in
//! the success colour, "Unchanged", or the error with its line in the error
//! colour; a check or an error icon before it and the run time after it.

use adw::prelude::*;
use gtk::glib;

use super::document::SourceChoice;
use super::palette;
use super::result::{ResultView, Segment, Status};

/// What the popup offers before the apps: no source app.
const NO_SOURCE: &str = "No source app";
/// SCR-07: why Save is off.
const SAVE_HINT: &str = "Fix the syntax error to save.";
/// How strongly a changed part of the link is tinted.
const MARK_ALPHA: &str = "22%";

/// The group's widgets.
#[derive(Debug, Clone)]
pub struct TestGroup {
    pub widget: adw::PreferencesGroup,
    pub link: adw::EntryRow,
    pub source: gtk::DropDown,
    sources: gtk::StringList,
    result: adw::ActionRow,
    icon: gtk::Image,
    time: gtk::Label,
    log: adw::ActionRow,
}

impl TestGroup {
    #[must_use]
    pub fn new() -> Self {
        let link = adw::EntryRow::builder()
            .title("Link")
            .input_purpose(gtk::InputPurpose::Url)
            .build();
        let sources = gtk::StringList::new(&[NO_SOURCE]);
        let source = gtk::DropDown::builder()
            .model(&sources)
            .valign(gtk::Align::Center)
            .tooltip_text("Source app: the app the link was opened from")
            .build();
        source.add_css_class("wye-script-source");
        source.update_property(&[gtk::accessible::Property::Label("Source app")]);
        link.add_suffix(&source);
        let icon = gtk::Image::builder()
            .pixel_size(16)
            .valign(gtk::Align::Center)
            .visible(false)
            .build();
        let time = gtk::Label::builder().valign(gtk::Align::Center).build();
        time.add_css_class("dimmed");
        time.add_css_class("caption");
        time.add_css_class("numeric");
        let result = adw::ActionRow::builder()
            .title("Result")
            .subtitle_selectable(true)
            .build();
        result.add_css_class("property");
        result.add_css_class("wye-script-result");
        result.add_prefix(&icon);
        result.add_suffix(&time);
        let log = adw::ActionRow::builder()
            .title("Log")
            .subtitle_selectable(true)
            .use_markup(false)
            .visible(false)
            .build();
        log.add_css_class("property");
        log.add_css_class("wye-script-log");
        let widget = adw::PreferencesGroup::builder().title("Test").build();
        widget.add(&link);
        widget.add(&result);
        widget.add(&log);
        let group = Self {
            widget,
            link,
            source,
            sources,
            result,
            icon,
            time,
            log,
        };
        group.show(&ResultView::default());
        group
    }

    /// Offer `choices` after "None", keeping the chosen app when it is still
    /// there; returns the desktop ID now chosen.
    pub fn set_choices(&self, choices: &[SourceChoice], chosen: &str) -> String {
        let names: Vec<&str> = std::iter::once(NO_SOURCE)
            .chain(choices.iter().map(|choice| choice.name.as_str()))
            .collect();
        self.sources.splice(0, self.sources.n_items(), &names);
        let index = choices
            .iter()
            .position(|choice| choice.id == chosen)
            .map_or(0, |index| index + 1);
        self.source.set_selected(u32::try_from(index).unwrap_or(0));
        choices
            .get(index.wrapping_sub(1))
            .map(|choice| choice.id.clone())
            .unwrap_or_default()
    }

    /// Show `view`.
    pub fn show(&self, view: &ResultView) {
        for class in ["changed", "unchanged", "failed"] {
            self.result.remove_css_class(class);
        }
        let (icon, class) = match view.status {
            Status::Idle => (None, None),
            Status::Changed => (Some("object-select-symbolic"), Some("changed")),
            Status::Unchanged => (Some("object-select-symbolic"), Some("unchanged")),
            Status::Failed => (Some("dialog-error-symbolic"), Some("failed")),
        };
        if let Some(class) = class {
            self.result.add_css_class(class);
        }
        self.icon.set_icon_name(icon);
        self.icon.set_visible(icon.is_some());
        self.result.set_subtitle(&subtitle(view));
        self.result
            .upcast_ref::<gtk::Widget>()
            .update_property(&[gtk::accessible::Property::Description(&plain(view))]);
        self.time.set_label(&view.time);
        self.time.set_visible(!view.time.is_empty());
        self.log.set_subtitle(&view.logs);
        self.log.set_visible(!view.logs.is_empty());
    }

    /// Follow the style manager: the marks are drawn in its colours.
    pub fn follow_style(&self, current: impl Fn() -> ResultView + 'static) {
        adw::StyleManager::default().connect_dark_notify(glib::clone!(
            #[weak(rename_to = result)]
            self.result,
            move |_| result.set_subtitle(&subtitle(&current()))
        ));
    }
}

impl Default for TestGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// The result row's value as Pango markup.
fn subtitle(view: &ResultView) -> String {
    match view.status {
        Status::Idle => "Not run yet".to_owned(),
        Status::Changed => marked(&view.segments, palette::current()),
        Status::Unchanged => "Unchanged".to_owned(),
        Status::Failed if view.syntax_error => format!(
            "{}\n<span size=\"small\">{SAVE_HINT}</span>",
            glib::markup_escape_text(&failure(view))
        ),
        Status::Failed => glib::markup_escape_text(&failure(view)).into(),
    }
}

/// The same in words, for screen readers.
fn plain(view: &ResultView) -> String {
    match view.status {
        Status::Idle => "Not run yet".to_owned(),
        Status::Changed => view
            .segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect(),
        Status::Unchanged => "Unchanged".to_owned(),
        Status::Failed if view.syntax_error => format!("{} {SAVE_HINT}", failure(view)),
        Status::Failed => failure(view),
    }
}

/// `Line 3: ReferenceError: …`, or the message alone without a line.
fn failure(view: &ResultView) -> String {
    if view.error_line > 0 {
        format!("Line {}: {}", view.error_line, view.message)
    } else {
        view.message.clone()
    }
}

/// The link with its changed parts bold and tinted in the success colour.
fn marked(segments: &[Segment], colours: palette::Palette) -> String {
    segments
        .iter()
        .map(|segment| {
            let text = glib::markup_escape_text(&segment.text);
            if segment.changed {
                format!(
                    "<span weight=\"bold\" foreground=\"{}\" background=\"{}\" bgalpha=\"{MARK_ALPHA}\">{text}</span>",
                    colours.success, colours.success_bg
                )
            } else {
                text.into()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scr_04_changed_parts_are_marked_and_escaped() {
        let markup = marked(
            &[
                Segment {
                    text: "https://".into(),
                    changed: false,
                },
                Segment {
                    text: "a&b".into(),
                    changed: true,
                },
            ],
            palette::of(false),
        );
        assert!(markup.starts_with("https://<span weight=\"bold\""));
        assert!(markup.contains(">a&amp;b</span>"));
    }

    #[test]
    fn scr_05_failures_name_their_line() {
        let view = ResultView {
            error_line: 3,
            ..ResultView::failed("ReferenceError: <x>")
        };
        assert_eq!(subtitle(&view), "Line 3: ReferenceError: &lt;x&gt;");
        assert_eq!(plain(&view), "Line 3: ReferenceError: <x>");
        assert_eq!(subtitle(&ResultView::failed("odd")), "odd");
        assert_eq!(subtitle(&ResultView::default()), "Not run yet");
        let syntax = ResultView {
            syntax_error: true,
            ..ResultView::failed("SyntaxError: x")
        };
        assert!(plain(&syntax).ends_with(SAVE_HINT), "SCR-07");
    }
}
