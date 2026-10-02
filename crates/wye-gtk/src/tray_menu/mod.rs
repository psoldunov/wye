//! The tray-menu popup for the toggle-menu shortcut and `wye menu`
//! (TRAY-08, 01-tray-menu.md): the tray's own menu as a GTK popover menu,
//! for sessions without a tray of their own to click (window managers). The
//! service calls `PickerHost1.ShowMenu` here (`crate::host`).
//!
//! The popover sits in a transparent layer surface covering the output, its
//! corner at the pointer when the service reported it, else in the middle
//! (`crate::overlay`); without layer shell it hangs from a tiny undecorated
//! window the window manager places. A second toggle, a click outside,
//! Escape or an item chosen closes it. Choosing an item sends its ID with
//! `ActivateTrayItem`, the tray's own dispatcher; `P` and `1`–`9` choose the
//! item with that shortcut (KEY-51).
//!
//! - [`model`]: the payload read into rows (crates/wye-ui/src/tray_menu,
//!   shared from source).
//! - [`menu`]: the rows as a `GMenu` with one action per row.

mod menu;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, so its nested `mod tests;` is found in
// `model/tests.rs`. `rows_json` is QML's only.
#[allow(
    dead_code,
    reason = "the file is shared whole; GTK reads the rows, not their JSON"
)]
pub mod model;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use self::model::MenuView;
use crate::app::Presenter;
use crate::overlay;
use crate::service;
use crate::widgets::menu_icons;

/// The layer-shell namespace: compositor rules can match it (README).
pub const NAMESPACE: &str = "wye-menu";

/// The popup surface: one window and one popover, reused.
pub struct TrayMenu {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for TrayMenu {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TrayMenu").finish_non_exhaustive()
    }
}

struct Inner {
    me: Weak<Inner>,
    window: gtk::Window,
    surface: gtk::Box,
    popover: gtk::PopoverMenu,
    view: RefCell<Option<MenuView>>,
    /// Action name to row ID, for the menu shown.
    ids: RefCell<HashMap<String, String>>,
    /// The compositor has focused the window since the menu opened.
    had_focus: Cell<bool>,
    /// Counts the times the menu opened. A `when_mapped` callback from an
    /// opening closed before its window mapped still fires on the next
    /// map; it runs only while its opening is the latest (TRAY-08).
    opened: Cell<u64>,
}

impl TrayMenu {
    pub fn new(app: &adw::Application) -> Self {
        Self {
            inner: Rc::new_cyclic(|weak: &Weak<Inner>| Inner::new(app, weak)),
        }
    }
}

impl Presenter for TrayMenu {
    /// `ShowMenu(menu)`: show the popup, or close it when it is shown.
    fn present(&self, _key: &str, argument: &str) {
        self.inner.toggle(argument);
    }
}

/// Escape, a click outside or a chosen item close the popover; the window
/// goes with it. KEY-51: `P` and `1`–`9` choose the top-level item with that
/// shortcut, before the menu's own keys; the window listens too, since a
/// keyboard that appears after the menu opened focuses the layer surface,
/// not the popover.
fn connect(window: &gtk::Window, popover: &gtk::PopoverMenu, inner: &Weak<Inner>) {
    let weak = inner.clone();
    popover.connect_closed(move |_| {
        if let Some(inner) = weak.upgrade() {
            inner.window.set_visible(false);
        }
    });
    popover.add_controller(keys(inner));
    window.add_controller(keys(inner));
    let weak = inner.clone();
    window.connect_close_request(move |_| {
        if let Some(inner) = weak.upgrade() {
            inner.popover.popdown();
        }
        glib::Propagation::Proceed
    });
    // A popover without autohide does not close on a click outside: another
    // window taking the focus closes it instead. Not `is-active`, which
    // also drops while a submenu of the menu has the keyboard: the
    // compositor keeps the window activated then.
    let weak = inner.clone();
    window.connect_realize(move |window| {
        let Some(toplevel) = window
            .surface()
            .and_then(|surface| surface.downcast::<gdk::Toplevel>().ok())
        else {
            return;
        };
        let weak = weak.clone();
        toplevel.connect_state_notify(move |toplevel| {
            let Some(inner) = weak.upgrade() else { return };
            if toplevel.state().contains(gdk::ToplevelState::FOCUSED) {
                inner.had_focus.set(true);
            } else if inner.had_focus.get() && !inner.popover.is_autohide() {
                inner.popover.popdown();
            }
        });
    });
}

fn keys(inner: &Weak<Inner>) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = inner.clone();
    keys.connect_key_pressed(move |_, keyval, _, modifiers| {
        let Some(inner) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if keyval == gdk::Key::Escape {
            inner.popover.popdown();
            return glib::Propagation::Stop;
        }
        if inner.accelerator(keyval, modifiers) {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    keys
}

impl Inner {
    fn new(app: &adw::Application, weak: &Weak<Self>) -> Self {
        let window = overlay::window(app, NAMESPACE, "Wye");
        window.set_hide_on_close(true);
        let surface = gtk::Box::builder().hexpand(true).vexpand(true).build();
        surface.add_css_class("wye-menu-surface");
        window.set_child(Some(&surface));
        let popover =
            gtk::PopoverMenu::from_model_full(&gio::Menu::new(), gtk::PopoverMenuFlags::NESTED);
        popover.set_has_arrow(false);
        popover.add_css_class("wye-tray-menu");
        popover.set_parent(&surface);
        if overlay::is_plain_wayland(&window) {
            // Mutter dismisses a grabbing popup at once unless it comes with
            // the serial of a fresh click or key in this client, and
            // `wye menu` or the toggle-menu shortcut gives it none: the menu
            // would never show (TRAY-08). Without the grab the window keeps
            // the keyboard and passes its keys to the menu, and losing the
            // focus closes it (`connect`).
            popover.set_autohide(false);
        }
        connect(&window, &popover, weak);
        Self {
            me: weak.clone(),
            window,
            surface,
            popover,
            view: RefCell::default(),
            ids: RefCell::default(),
            had_focus: Cell::new(false),
            opened: Cell::new(0),
        }
    }

    /// Show the payload `json`, or hide the popup when it is shown.
    fn toggle(&self, json: &str) {
        if self.window.is_visible() {
            self.popover.popdown();
            self.window.set_visible(false);
            return;
        }
        let view = match MenuView::parse(json) {
            Ok(view) => view,
            Err(error) => {
                // `ShowMenu` refuses what cannot be read.
                tracing::warn!(%error, "cannot show the tray menu");
                return;
            }
        };
        let built = menu::build(&view.rows);
        let group = gio::SimpleActionGroup::new();
        for action in &built.actions {
            let weak = self.me.clone();
            action.connect_activate(move |action, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.activate_action(&action.name());
                }
            });
            group.add_action(action);
        }
        self.window.insert_action_group(menu::GROUP, Some(&group));
        self.popover.set_menu_model(Some(&built.menu));
        menu_icons::add(&self.popover, &built.icons);
        if let Some((width, height)) = self.menu_size().filter(|_| self.covers_window()) {
            // The window takes the menu's size, so the compositor keeps all
            // of it on screen wherever it puts the window, and the menu
            // covers it (`pop_up`).
            self.window.set_default_size(width.max(1), height.max(1));
        }
        *self.ids.borrow_mut() = built.ids;
        let placement = view.placement.clone();
        *self.view.borrow_mut() = Some(view);
        let monitor = placement
            .as_ref()
            .and_then(|at| overlay::monitor_named(&at.output));
        overlay::set_monitor(&self.window, monitor.as_ref());
        self.had_focus.set(false);
        self.window.present();
        if let Some((at, monitor)) = placement.as_ref().zip(monitor.as_ref()) {
            // On X11 the menu hangs from the window: put it at the pointer.
            overlay::place_window(&self.window, (at.x, at.y), monitor, false);
        }
        // The popover needs its parent on screen: a popup asked for before
        // the compositor has mapped the window is never shown (TRAY-08).
        let opening = self.opened.get().wrapping_add(1);
        self.opened.set(opening);
        let weak = self.me.clone();
        overlay::when_mapped(&self.window, move || {
            if let Some(inner) = weak.upgrade().filter(|inner| inner.opened.get() == opening) {
                inner.pop_up(placement.as_ref().map(|at| (at.x, at.y)), monitor.as_ref());
            }
        });
    }

    /// Open the popover with its corner at `pointer` (the output's logical
    /// coordinates), or in the middle of the output.
    fn pop_up(&self, pointer: Option<(i32, i32)>, monitor: Option<&gdk::Monitor>) {
        if !self.window.is_visible() {
            return;
        }
        if overlay::is_layer(&self.window) {
            let (width, height) = overlay::output_size(monitor)
                .unwrap_or((self.surface.width(), self.surface.height()));
            let rect = if let Some((x, y)) = pointer.filter(|_| monitor.is_some()) {
                self.popover.set_halign(gtk::Align::Start);
                gdk::Rectangle::new(x, y, 1, 1)
            } else {
                self.popover.set_halign(gtk::Align::Center);
                gdk::Rectangle::new(width / 2, height / 2, 1, 1)
            };
            self.popover.set_pointing_to(Some(&rect));
            self.popover.set_position(gtk::PositionType::Bottom);
            self.popover.popup();
            if pointer.is_none() || monitor.is_none() {
                // Centred: lift it by half its height, which it has once
                // shown (a hidden widget measures nothing), before it is
                // first laid out.
                let menu_height = self.popover.measure(gtk::Orientation::Vertical, -1).1;
                let top = ((height - menu_height) / 2).max(0);
                self.popover
                    .set_pointing_to(Some(&gdk::Rectangle::new(width / 2, top, 1, 1)));
            }
            return;
        }
        if self.covers_window() || (pointer.is_some() && monitor.is_some()) {
            // The window's corner is at the pointer (X11), or the window is
            // the menu's size (Wayland without layer shell): hang from it.
            self.popover.set_halign(gtk::Align::Start);
            self.popover
                .set_pointing_to(Some(&gdk::Rectangle::new(0, 0, 1, 1)));
        }
        self.popover.set_position(gtk::PositionType::Bottom);
        self.popover.popup();
        if !self.popover.is_autohide() && self.popover.focus_child().is_none() {
            // TRAY-08: the arrows and Enter work at once, as GTK arranges
            // for a popover with autohide.
            self.popover.child_focus(gtk::DirectionType::TabForward);
        }
    }

    /// Whether the menu covers its window, sized to it: Wayland without
    /// layer shell, where the compositor places the window and nothing
    /// tells where (GNOME with the Shell extension off).
    fn covers_window(&self) -> bool {
        overlay::is_plain_wayland(&self.window)
    }

    /// The menu's natural size, measured while the popover is hidden (its
    /// contents are visible widgets of their own).
    fn menu_size(&self) -> Option<(i32, i32)> {
        let contents = self.popover.child()?.parent()?;
        let width = contents.measure(gtk::Orientation::Horizontal, -1).1;
        let height = contents.measure(gtk::Orientation::Vertical, width).1;
        Some((width, height))
    }

    /// KEY-51: the top-level row whose shortcut is the key pressed.
    fn accelerator(&self, keyval: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let plain = !modifiers.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        );
        let Some(text) = keyval.to_unicode().filter(|_| plain).map(String::from) else {
            return false;
        };
        let id = self
            .view
            .borrow()
            .as_ref()
            .and_then(|view| view.accelerator(&text).map(str::to_owned));
        id.is_some_and(|id| self.activate(&id))
    }

    fn activate_action(&self, name: &str) {
        let id = self.ids.borrow().get(name).cloned();
        if let Some(id) = id {
            self.activate(&id);
        }
    }

    /// Send row `id` to the service and close; false when it cannot be
    /// chosen.
    fn activate(&self, id: &str) -> bool {
        let selectable = self
            .view
            .borrow()
            .as_ref()
            .is_some_and(|view| view.is_selectable(id));
        if !selectable {
            return false;
        }
        let id = id.to_owned();
        service::request(
            move |proxy| async move { proxy.activate_tray_item(&id).await },
            |result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the service did not run the menu item");
                }
            },
        );
        self.popover.popdown();
        true
    }
}
