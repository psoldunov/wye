//! Toasts: libadwaita's short message at the bottom of a window, with an
//! optional button ("Rule deleted · Undo", RUL-06; "Rules exported",
//! RUL-02). The GNOME form of KDE's inline undo message.
//!
//! A window gets its `AdwToastOverlay` the first time a toast is shown in
//! it: the overlay takes the window's content as its child, so no window
//! has to be built with one.
//!
//! API:
//! - [`show`]`(widget, toast)`: show `toast` in the window `widget` is in.

use adw::prelude::*;

/// Show `toast` over the window `widget` belongs to; nothing when it is in
/// no `AdwApplicationWindow` (yet).
pub fn show(widget: &impl IsA<gtk::Widget>, toast: adw::Toast) {
    let Some(window) = widget.root().and_downcast::<adw::ApplicationWindow>() else {
        tracing::debug!("a toast for a widget outside an application window");
        return;
    };
    overlay(&window).add_toast(toast);
}

/// The window's toast overlay, put around its content the first time.
fn overlay(window: &adw::ApplicationWindow) -> adw::ToastOverlay {
    if let Some(overlay) = window.content().and_downcast::<adw::ToastOverlay>() {
        return overlay;
    }
    let content = window.content();
    window.set_content(None::<&gtk::Widget>);
    let overlay = adw::ToastOverlay::new();
    overlay.set_child(content.as_ref());
    window.set_content(Some(&overlay));
    overlay
}
