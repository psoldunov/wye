//! BLK-17 Inline links: `<a href>` in a subtitle, a group description, a
//! callout or a status page opens through Wye's own pipeline, like any
//! other link, instead of GTK's default handler.
//!
//! API:
//! - [`route_links`]`(widget, open)`: every `GtkLabel` inside `widget` (its
//!   own subtitle labels included) hands an activated link to `open` and
//!   reports it handled. Call it once the widget is built; labels added later
//!   need another call. `open` is usually `store.open_link`. A label with a
//!   link gets `wye-links`: its dimmed text is drawn in a solid colour, so
//!   its links keep the full accent colour (an opacity or a translucent
//!   colour would dim them too).
//! - [`link_label`]`(markup, open)`: a wrapping, dimmed label with links,
//!   for notes under a group.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

/// Route the links of every label in `widget` (and `widget` itself) to
/// `open`.
pub fn route_links(widget: &impl IsA<gtk::Widget>, open: impl Fn(&str) + 'static) {
    let open: Rc<dyn Fn(&str)> = Rc::new(open);
    walk(widget.upcast_ref(), &open);
}

fn walk(widget: &gtk::Widget, open: &Rc<dyn Fn(&str)>) {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        if label.uses_markup() && label.label().contains("<a ") {
            label.add_css_class("wye-links");
        }
        let open = Rc::clone(open);
        label.connect_activate_link(move |_, uri| {
            open(uri);
            glib::Propagation::Stop
        });
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        walk(&widget, open);
        child = widget.next_sibling();
    }
}

/// A wrapping, dimmed label showing `markup`, its links sent to `open`.
#[must_use]
pub fn link_label(markup: &str, open: impl Fn(&str) + 'static) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(markup)
        .use_markup(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .css_classes(["dimmed"])
        .build();
    route_links(&label, open);
    label
}
