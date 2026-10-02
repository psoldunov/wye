//! The window the picker and the tray-menu popup draw in (02-picker.md,
//! "Linux notes"; TRAY-08).
//!
//! - Where the compositor offers `zwlr_layer_shell_v1` (Sway, Hyprland,
//!   niri, river, Wayfire, KDE): a transparent layer surface on the overlay
//!   layer covering the output, with exclusive keyboard focus, so hotkeys
//!   work at once and a click outside the panel lands on the surface
//!   (PICK-23). The panel sits at the pointer when the service reported it,
//!   else in the middle of the output; the output is the reported one, else
//!   the compositor's choice (the focused one). The namespace (`wye-picker`,
//!   `wye-menu`) lets compositor rules blur it or switch off its animation
//!   (PICK-01, PICK-15).
//! - On X11: an undecorated window holding only the panel, centred on the
//!   pointer the service reported and kept on its monitor, as crates/wye-ui
//!   places its panel ([`place_window`], through the window's X11 ID).
//! - Elsewhere (GNOME without the Shell extension): an undecorated window
//!   holding only the panel, placed by the compositor.

mod x11;

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell as _};

/// Room between the panel and the edge of the output (PICK-02).
pub const EDGE_MARGIN: i32 = 12;

/// Whether the session can show layer surfaces. Only a Wayland display is
/// asked: gtk4-layer-shell asserts on any other.
pub fn layer_shell() -> bool {
    let wayland =
        gdk::Display::default().is_some_and(|display| display.type_().name().contains("Wayland"));
    wayland && gtk4_layer_shell::is_supported()
}

/// A new, hidden overlay window for `app` with the layer-shell `namespace`.
pub fn window(app: &adw::Application, namespace: &str, title: &str) -> gtk::Window {
    let window = gtk::Window::builder()
        .application(app)
        .title(title)
        .decorated(false)
        .build();
    window.add_css_class("wye-overlay");
    if layer_shell() {
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace(Some(namespace));
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_exclusive_zone(-1);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
    } else {
        // A layer surface takes the output's size from the compositor; a
        // plain window is as large as the panel.
        window.set_resizable(false);
        window.add_css_class("wye-windowed");
    }
    window
}

/// Whether `window` is a layer surface (see the module docs).
pub fn is_layer(window: &gtk::Window) -> bool {
    window.is_layer_window()
}

/// Whether `window` is a plain Wayland window: Wayland without layer shell
/// (GNOME with the Shell extension off).
pub fn is_plain_wayland(window: &gtk::Window) -> bool {
    !is_layer(window)
        && WidgetExt::display(window)
            .type_()
            .name()
            .contains("Wayland")
}

/// Run `run` once `window`, which has just been presented, is on screen: a
/// popup needs its parent there. GDK calls a surface mapped as soon as the
/// compositor configures it, before its first frame is committed, and
/// Mutter dismisses a popup whose parent has no frame yet; so after the
/// first frame drawn once mapped. On the next main-loop turn when the
/// window is on screen already. A window hidden before it maps runs `run`
/// on its next map, beside that showing's own: the caller tells them apart.
pub fn when_mapped(window: &gtk::Window, run: impl FnOnce() + 'static) {
    let Some(surface) = window.surface().filter(|surface| !surface.is_mapped()) else {
        glib::idle_add_local_once(run);
        return;
    };
    let run = Cell::new(Some(run));
    once(
        &surface,
        |surface, handler| {
            surface.connect_mapped_notify(move |surface| {
                if surface.is_mapped() {
                    handler(surface);
                }
            })
        },
        move |surface: &gdk::Surface| {
            let run = Cell::new(run.take());
            once(
                &surface.frame_clock(),
                |clock, handler| clock.connect_after_paint(move |clock| handler(clock)),
                move |_| {
                    if let Some(run) = run.take() {
                        // After the frame's commit has gone out.
                        glib::idle_add_local_once(run);
                    }
                },
            );
        },
    );
}

/// Connect `run` to `object` with `connect`, for its first emission only.
fn once<T: IsA<glib::Object>>(
    object: &T,
    connect: impl FnOnce(&T, Box<dyn Fn(&T)>) -> glib::SignalHandlerId,
    run: impl FnOnce(&T) + 'static,
) {
    let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
    let own = Rc::clone(&handler);
    let run = Cell::new(Some(run));
    let id = connect(
        object,
        Box::new(move |object: &T| {
            if let Some(id) = own.take() {
                object.disconnect(id);
            }
            if let Some(run) = run.take() {
                run(object);
            }
        }),
    );
    handler.set(Some(id));
}

/// The monitor whose connector is `output` (`DP-1`), if there is one.
pub fn monitor_named(output: &str) -> Option<gdk::Monitor> {
    if output.is_empty() {
        return None;
    }
    let monitors = gdk::Display::default()?.monitors();
    (0..monitors.n_items())
        .filter_map(|index| monitors.item(index)?.downcast::<gdk::Monitor>().ok())
        .find(|monitor| monitor.connector().is_some_and(|name| name == output))
}

/// Put a layer window on `monitor`, or let the compositor choose.
pub fn set_monitor(window: &gtk::Window, monitor: Option<&gdk::Monitor>) {
    if is_layer(window) {
        window.set_monitor(monitor);
    }
}

/// The output a layer window will cover, for sizes before it is shown:
/// `monitor`, else the first one.
pub fn output_size(monitor: Option<&gdk::Monitor>) -> Option<(i32, i32)> {
    let monitor = monitor.cloned().or_else(|| {
        gdk::Display::default()?
            .monitors()
            .item(0)?
            .downcast::<gdk::Monitor>()
            .ok()
    })?;
    let geometry = monitor.geometry();
    Some((geometry.width(), geometry.height()))
}

/// PICK-02 for a window that is not a layer surface: on X11, `window`
/// centred on the reported pointer `at` (pixels of `monitor`, as the
/// service's X11 probe reads them) and kept [`EDGE_MARGIN`] inside the
/// monitor, or with its corner there when `centred` is false (the tray-menu
/// popup's window, whose menu hangs from it). Elsewhere nothing happens.
pub fn place_window(window: &gtk::Window, at: (i32, i32), monitor: &gdk::Monitor, centred: bool) {
    if is_layer(window) {
        return;
    }
    let scale = monitor.scale_factor().max(1);
    let geometry = monitor.geometry();
    let (x, y) = (at.0 / scale, at.1 / scale);
    let (left, top) = if centred {
        let width = window.measure(gtk::Orientation::Horizontal, -1).1;
        let height = window.measure(gtk::Orientation::Vertical, width).1;
        (
            centred_on(x, width, geometry.width()),
            centred_on(y, height, geometry.height()),
        )
    } else {
        (
            x.clamp(0, (geometry.width() - 1).max(0)),
            y.clamp(0, (geometry.height() - 1).max(0)),
        )
    };
    x11::place(
        window,
        (geometry.x() + left) * scale,
        (geometry.y() + top) * scale,
    );
}

/// Where a `size` box centred on `point` goes inside `room`, kept
/// [`EDGE_MARGIN`] from every edge (PICK-02). Each axis on its own.
pub fn centred_on(point: i32, size: i32, room: i32) -> i32 {
    let high = (room - size - EDGE_MARGIN).max(EDGE_MARGIN);
    (point - size / 2).clamp(EDGE_MARGIN, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_centres_on_the_pointer_and_stays_on_the_output() {
        // PICK-02.
        assert_eq!(centred_on(500, 200, 1920), 400);
        assert_eq!(centred_on(10, 200, 1920), EDGE_MARGIN);
        assert_eq!(centred_on(1910, 200, 1920), 1920 - 200 - EDGE_MARGIN);
        // Larger than the output: keep the start on screen.
        assert_eq!(centred_on(100, 3000, 1920), EDGE_MARGIN);
    }
}
