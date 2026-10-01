//! Moving a window on X11 (PICK-02). GTK 4 has no API for a window's
//! position, so it goes through the window's ID on a connection of its own:
//! the position becomes the user-specified position hint (ICCCM
//! `USPosition`), which window managers honour when they map the window,
//! and a move request, for a window already shown. Once the window is
//! mapped the move is asked again, for a window manager that placed it
//! anyway. One connection serves every move, opened on the first: setting it
//! up each time would cost the main loop a handshake per picker. On any
//! other display nothing happens.

use std::cell::RefCell;
use std::rc::Rc;

use gdk4_x11::X11Surface;
use gtk::prelude::*;
use x11rb::connection::Connection as _;
use x11rb::properties::{WmSizeHints, WmSizeHintsSpecification};
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

thread_local! {
    /// The connection of [`request_move`], with the display it is for; the
    /// main thread's alone, as GTK's.
    static CONNECTION: RefCell<Option<(String, RustConnection)>> = const { RefCell::new(None) };
}

/// Put the top-left corner of `window` at (`x`, `y`) of the X screen, in
/// device pixels.
pub fn place(window: &gtk::Window, x: i32, y: i32) {
    if !window.is_realized() {
        WidgetExt::realize(window);
    }
    let Some(surface) = window.surface() else {
        return;
    };
    let Some(xid) = surface.downcast_ref::<X11Surface>().map(X11Surface::xid) else {
        return;
    };
    let display = surface.display();
    // GTK's own size hints reach the server first, so they are kept.
    display.sync();
    let name = display.name().to_string();
    move_to(&name, xid, x, y);
    if surface.is_mapped() {
        return;
    }
    let handler = Rc::new(RefCell::new(None));
    let id = surface.connect_mapped_notify({
        let handler = Rc::clone(&handler);
        move |surface| {
            if !surface.is_mapped() {
                return;
            }
            move_to(&name, xid, x, y);
            if let Some(id) = handler.borrow_mut().take() {
                surface.disconnect(id);
            }
        }
    });
    handler.replace(Some(id));
}

/// Ask for window `xid` at (`x`, `y`); a failure only leaves it where the
/// window manager put it.
fn move_to(display: &str, xid: u64, x: i32, y: i32) {
    match request_move(display, xid, x, y) {
        Ok(()) => tracing::debug!(xid, x, y, "placed the window on X11"),
        Err(error) => tracing::debug!(%error, "cannot place the window on X11"),
    }
}

fn request_move(display: &str, xid: u64, x: i32, y: i32) -> anyhow::Result<()> {
    CONNECTION.with_borrow_mut(|cached| {
        if cached.as_ref().is_none_or(|(name, _)| name != display) {
            let (connection, _) = x11rb::connect(Some(display))?;
            *cached = Some((display.to_owned(), connection));
        }
        let Some((_, connection)) = cached.as_ref() else {
            return Ok(());
        };
        let moved = move_window(connection, xid, x, y);
        if moved.is_err() {
            // A broken connection is opened again next time.
            *cached = None;
        }
        moved
    })
}

fn move_window(connection: &RustConnection, xid: u64, x: i32, y: i32) -> anyhow::Result<()> {
    let window = u32::try_from(xid)?;
    let mut hints = WmSizeHints::get_normal_hints(connection, window)?
        .reply()?
        .unwrap_or_default();
    hints.position = Some((WmSizeHintsSpecification::UserSpecified, x, y));
    hints.set_normal_hints(connection, window)?.check()?;
    connection
        .configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?
        .check()?;
    connection.flush()?;
    Ok(())
}
