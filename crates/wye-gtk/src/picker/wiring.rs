//! The picker window's signals, connected once (02-picker.md
//! "Interaction"): keys, focus, clicks outside the panel and the menus'
//! actions.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use super::{Inner, gdk_keys, menus, point};

impl Inner {
    /// Keys, focus, clicks outside the panel and the menus' actions.
    pub(super) fn connect(this: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        // Before any widget: Tab, Return and the arrows are the picker's.
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        keys.connect_key_pressed(move |_, keyval, keycode, modifiers| {
            weak.upgrade().map_or(glib::Propagation::Proceed, |inner| {
                inner.key(keyval, keycode, modifiers)
            })
        });
        // KEY-13: a modifier key's release changes what is held (its press
        // is handled with the other keys).
        let weak = Rc::downgrade(this);
        keys.connect_key_released(move |_, keyval, _, modifiers| {
            if let Some(inner) = weak.upgrade() {
                inner.set_held(gdk_keys::after(keyval, modifiers, false));
            }
        });
        this.window.add_controller(keys);
        Self::connect_focus(this);
        Self::connect_outside(this);
        Self::add_actions(this);
    }

    /// PICK-23: losing the focus cancels, unless a menu of the picker has
    /// it.
    fn connect_focus(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.window.connect_is_active_notify(move |window| {
            let Some(inner) = weak.upgrade() else { return };
            if window.is_active() {
                inner.was_active.set(true);
            } else if inner.was_active.get() && window.is_visible() && !inner.menu_open() {
                inner.cancel();
            }
        });
        let weak = Rc::downgrade(this);
        this.window.connect_close_request(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.cancel();
            }
            glib::Propagation::Proceed
        });
        Self::connect_stale_focus(this);
        for menu in [&this.overflow, &this.tile_menu] {
            let weak = Rc::downgrade(this);
            menu.connect_closed(move |menu| {
                let Some(inner) = weak.upgrade() else { return };
                if menu == &inner.overflow {
                    inner.hush_more_tooltip();
                }
                // The menu closed because another window took the focus. A
                // new request (PICK-27) closes the menu too: the request it
                // was opened for is the only one this may cancel.
                let request_id = inner.request_id.borrow().clone();
                glib::idle_add_local_once(move || {
                    let same = *inner.request_id.borrow() == request_id;
                    if same && inner.was_active.get() && !inner.window.is_active() {
                        inner.cancel();
                    }
                });
            });
        }
    }

    /// PICK-23, KEY-22: a menu closed from a submenu page (Open In) leaves
    /// the window's focus on a widget of the hidden popover: GTK moves it
    /// there after the popover has closed, as the popover turns back to its
    /// main page. GTK drops every key aimed at an unmapped widget, so the
    /// picker's own keys would stop until a menu opened again. Whenever the
    /// focus lands on a widget that is not on screen, give it back to the
    /// window.
    fn connect_stale_focus(this: &Rc<Self>) {
        this.window.connect_focus_widget_notify(|window| {
            if !has_stale_focus(window) {
                return;
            }
            // Not from inside the notification that set it.
            let window = window.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(window) = window.upgrade().filter(has_stale_focus) {
                    GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
                }
            });
        });
    }

    /// PICK-23: on a layer surface the panel floats in a transparent
    /// surface covering the output; a click there cancels.
    fn connect_outside(this: &Rc<Self>) {
        let click = gtk::GestureClick::builder().button(0).build();
        let weak = Rc::downgrade(this);
        click.connect_pressed(move |_, _, x, y| {
            let Some(inner) = weak.upgrade() else { return };
            let inside = inner
                .panel
                .compute_bounds(&inner.surface)
                .is_some_and(|bounds| bounds.contains_point(&point(x, y)));
            if !inside {
                inner.cancel();
            }
        });
        this.surface.add_controller(click);
    }

    fn add_actions(this: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        let string = Some(glib::VariantTy::STRING);
        let with_target = |name: &str, run: fn(&Self, &str)| {
            let action = gio::SimpleAction::new(name, string);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, target| {
                let target = target.and_then(glib::Variant::str).unwrap_or_default();
                if let Some(inner) = weak.upgrade() {
                    run(&inner, target);
                }
            });
            group.add_action(&action);
        };
        with_target(menus::OPEN_IN, Self::open_in);
        with_target(menus::TILE, Self::tile_action);
        for name in [menus::COPY_LINK, menus::CREATE_RULE, menus::SETTINGS] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |action, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.overflow_action(&action.name());
                }
            });
            group.add_action(&action);
        }
        this.window.insert_action_group(menus::GROUP, Some(&group));
    }
}

/// Whether `window`'s focus is on a widget that is not on screen (see
/// `Inner::connect_stale_focus`).
fn has_stale_focus(window: &gtk::Window) -> bool {
    GtkWindowExt::focus(window).is_some_and(|focus| !focus.is_mapped())
}
