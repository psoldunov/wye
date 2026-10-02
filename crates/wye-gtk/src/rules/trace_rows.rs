//! The rule tester's Steps card (DLG-TST-02): one row per pipeline step
//! that changed the link or decided the target, a dimmed label in a column
//! of its own ("Expanded", "Cleaned", "Matched"), then what happened; the
//! last row is the outcome: where the link opens, with the target's icon,
//! its options and the final link, or the picker, or why it was refused.
//!
//! The label column is one width for every row of the card (a
//! `GtkSizeGroup`), so each step's text starts at the same place.
//!
//! KDE counterpart: crates/wye-ui/qml/rules/TesterRow.qml and the Steps
//! card of RuleTesterSheet.qml.

use adw::prelude::*;

use super::tester::TraceView;
use crate::settings::menu::Row as TargetLabel;
use crate::widgets::icon;

/// The outcome's icon size: an app's icon, and the smaller symbolic glyph
/// of the picker or a refusal.
const OUTCOME_ICON_SIZE: i32 = 32;
const SYMBOL_SIZE: i32 = 24;

/// How the outcome reads, by `TraceView::decision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// `open`: "Opens in" the target.
    Opens,
    /// `picker`: "Shows" the picker.
    Picker,
    /// `rejected`: "Refused", and why.
    Refused,
}

impl Outcome {
    /// The outcome of `decision`.
    #[must_use]
    pub fn of(decision: &str) -> Self {
        match decision {
            "picker" => Self::Picker,
            "rejected" => Self::Refused,
            _ => Self::Opens,
        }
    }

    /// The row's label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Opens => "Opens in",
            Self::Picker => "Shows",
            Self::Refused => "Refused",
        }
    }
}

/// A row with `label` in the label column and `content` after it.
fn labelled(
    label: &str,
    emphasis: bool,
    content: &gtk::Widget,
    labels: &gtk::SizeGroup,
) -> gtk::ListBoxRow {
    let title = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .yalign(0.0)
        .valign(gtk::Align::Start)
        .build();
    if !emphasis {
        title.add_css_class("dimmed");
    }
    labels.add_widget(&title);
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .css_classes(["wye-trace-row"])
        .build();
    line.append(&title);
    line.append(content);
    let row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&line)
        .build();
    row.update_property(&[gtk::accessible::Property::Label(label)]);
    row
}

/// One step: its label and what happened.
#[must_use]
pub fn step(label: &str, text: &str, labels: &gtk::SizeGroup) -> gtk::ListBoxRow {
    let what = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .hexpand(true)
        .selectable(true)
        .build();
    labelled(label, false, what.upcast_ref(), labels)
}

/// The outcome of `view`; `target` names its target (TGT-01).
#[must_use]
pub fn outcome(view: &TraceView, target: &TargetLabel, labels: &gtk::SizeGroup) -> gtk::ListBoxRow {
    let outcome = Outcome::of(&view.decision);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .hexpand(true)
        .build();
    let glyph: gtk::Widget = match outcome {
        Outcome::Refused => {
            let image = gtk::Image::builder()
                .icon_name("dialog-error-symbolic")
                .pixel_size(SYMBOL_SIZE)
                .css_classes(["error"])
                .build();
            image.upcast()
        }
        Outcome::Picker => icon::image("view-list-bullet-symbolic", SYMBOL_SIZE).upcast(),
        Outcome::Opens => {
            let source = if target.icon.is_empty() {
                "application-x-executable"
            } else {
                target.icon.as_str()
            };
            icon::target_icon(source, target.badge.as_ref(), OUTCOME_ICON_SIZE)
        }
    };
    glyph.set_valign(gtk::Align::Start);
    content.append(&glyph);

    let lines = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    let name = match outcome {
        Outcome::Refused => view.rejected.clone(),
        Outcome::Picker => "the picker".to_owned(),
        Outcome::Opens if view.target_name.is_empty() => target.label.clone(),
        Outcome::Opens => view.target_name.clone(),
    };
    let headline = gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["heading"])
        .build();
    if outcome == Outcome::Refused {
        headline.add_css_class("error");
    }
    lines.append(&headline);
    for (text, wrap) in [
        (&view.options, gtk::pango::WrapMode::Word),
        (&view.final_url, gtk::pango::WrapMode::Char),
    ] {
        if text.is_empty() {
            continue;
        }
        let detail = gtk::Label::builder()
            .label(text)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(wrap)
            .selectable(true)
            .css_classes(["dimmed", "caption"])
            .build();
        lines.append(&detail);
    }
    content.append(&lines);
    labelled(outcome.label(), true, content.upcast_ref(), labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlg_tst_02_the_outcome_reads_by_decision() {
        assert_eq!(Outcome::of("open").label(), "Opens in");
        assert_eq!(Outcome::of("picker").label(), "Shows");
        assert_eq!(Outcome::of("rejected").label(), "Refused");
        assert_eq!(Outcome::of("").label(), "Opens in");
    }
}
