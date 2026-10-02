//! The Reference (SCR-06): the script API and the two examples of
//! 16-script-editor.md "Script API", in the sidebar beside the code. The
//! signature and the examples are highlighted as the code is; each example
//! has a Copy button. The parameters read as a list rather than one wide
//! `JSDoc` block, so nothing runs off the narrow pane.
//!
//! Same copy as crates/wye-ui/qml/script/ScriptReference.qml.

use adw::prelude::*;
use gtk::glib;

use sourceview5::prelude::*;

use super::code;
use crate::widgets::toast;

/// How the pane opens.
const INTRO: &str = "The script exports a default function. It receives the link after \
expansion and cleaning as a URL object, which it may change, and a context. Return a URL or a \
string to open a different link, or nothing to keep it. The result must be an http or https link.";

/// The function every script exports.
const SIGNATURE: &str = "export default function transform(url, context) {}";

/// The parameters and the return value: name, type, meaning.
const PARAMETERS: [(&str, &str, &str); 6] = [
    (
        "url",
        "URL",
        "The link after expansion and cleaning. Mutable.",
    ),
    (
        "context.sourceApp",
        "string or null",
        "Desktop ID of the source app, if known.",
    ),
    (
        "context.entryPoint",
        "string",
        "\"handler\", \"clipboard\", \"extension\" or \"cli\".",
    ),
    (
        "context.heldKeys",
        "string[]",
        "The keys held, e.g. [\"Ctrl\"].",
    ),
    (
        "context.rule",
        "string or null",
        "The matched rule's name (per-rule scripts only).",
    ),
    (
        "Returns",
        "URL, string or undefined",
        "The new link, or undefined to keep it.",
    ),
];

/// What a script may use (SCR-20, SCR-21).
const LIMITS: &str = "Available: URL, URLSearchParams and console.log, which writes to Wye's \
log and to the Test group. There is no network, file access or timers. A run may take 50 ms and \
16 MB of memory.";

/// The examples: what each does, and its code.
const EXAMPLES: [(&str, &str); 2] = [
    (
        "Open Reddit links on old.reddit.com",
        "export default function transform(url) {\n  if (url.hostname.endsWith(\"reddit.com\"))\n    url.hostname = \"old.reddit.com\";\n  return url;\n}",
    ),
    (
        "Send YouTube links to a self-hosted front end",
        "export default function transform(url) {\n  if (url.hostname !== \"youtu.be\") return;\n  const id = url.pathname.slice(1);\n  const host = \"invidious.example.org\";\n  return `https://${host}/watch?v=${id}`;\n}",
    ),
];

/// The pane: a scrolled column, for the split view's sidebar.
#[must_use]
pub fn pane() -> gtk::ScrolledWindow {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(24)
        .margin_start(14)
        .margin_end(14)
        .build();
    column.append(&heading("Script API"));
    column.append(&paragraph(INTRO));
    column.append(&sample(SIGNATURE));
    column.append(&parameters());
    column.append(&paragraph(LIMITS));
    let examples = heading("Examples");
    examples.set_margin_top(12);
    column.append(&examples);
    for (title, text) in EXAMPLES {
        column.append(&example(title, text));
    }
    let pane = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&column)
        .build();
    pane.add_css_class("wye-script-reference");
    pane.update_property(&[gtk::accessible::Property::Label("Script Reference")]);
    pane
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("heading");
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    label
}

fn paragraph(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .build()
}

/// A read-only, highlighted code sample. The examples' lines are short
/// enough for the pane; a longer one wraps.
fn sample(text: &str) -> gtk::Widget {
    let buffer = code::buffer();
    buffer.set_text(text);
    buffer.set_highlight_matching_brackets(false);
    let view = sourceview5::View::builder()
        .buffer(&buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(10)
        .bottom_margin(10)
        .left_margin(12)
        .right_margin(12)
        .build();
    view.upcast_ref::<gtk::Widget>()
        .update_property(&[gtk::accessible::Property::Label("Code sample")]);
    view.add_css_class("wye-script-sample");
    view.set_overflow(gtk::Overflow::Hidden);
    view.upcast()
}

/// The parameters as property rows: the name in monospace, its type and
/// meaning below.
fn parameters() -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    for (name, kind, meaning) in PARAMETERS {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(name))
            .subtitle(glib::markup_escape_text(&format!("{kind} · {meaning}")))
            .activatable(false)
            .build();
        row.add_css_class("wye-script-parameter");
        list.append(&row);
    }
    list
}

/// One example: what it does, a Copy button, and its code.
fn example(title: &str, text: &str) -> gtk::Box {
    let label = gtk::Label::builder()
        .label(title)
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .build();
    label.add_css_class("dimmed");
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy Example")
        .valign(gtk::Align::Center)
        .build();
    copy.add_css_class("flat");
    copy.update_property(&[gtk::accessible::Property::Label(&format!("Copy “{title}”"))]);
    let owned = text.to_owned();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(&owned);
        toast::show(button, adw::Toast::new("Example copied"));
    });
    let top = gtk::Box::builder().spacing(6).build();
    top.append(&label);
    top.append(&copy);
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    card.append(&top);
    card.append(&sample(text));
    card
}
