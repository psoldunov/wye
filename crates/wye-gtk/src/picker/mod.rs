//! The picker (02-picker.md), for sessions without the GNOME Shell
//! extension: window managers and wlroots compositors (ADV-12, `gnome`
//! frontend). The service calls `PickerHost1.ShowPicker` here
//! (`crate::host`); the answer goes back with `PickerChose`,
//! `PickerCancelled` or `PickerAction` (PIPE-13).
//!
//! The logic is crates/wye-ui's, shared from source so both pickers behave
//! alike:
//!
//! - [`view`]: a `PickerRequest` read into tiles, keymap, metrics and the
//!   URL line (symlink to crates/wye-ui/src/picker/view.rs).
//! - [`state`]: selection, held modifiers and what each input does (symlink
//!   to crates/wye-ui/src/picker/state.rs, with its tests).
//! - [`keys`]: a key press as the core keymap reads it (symlink to
//!   crates/wye-ui/src/picker/keys.rs).
//! - [`model`]: tiles and the Open In list as plain data
//!   (crates/wye-ui/src/picker/qml.rs).
//!
//! On GTK:
//!
//! - [`gdk_keys`]: GDK key presses in the shape [`keys`] reads.
//! - [`panel`], [`tile`]: the panel, drawn like the Shell extension's.
//! - [`menus`]: the "⋯" menu and the tile menu as `GMenu` models.
//! - [`answer`]: the calls to the service and the activation token.
//! - `crate::overlay`: the layer-shell surface, or a plain window.

mod answer;
#[cfg(test)]
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/picker/fixture.rs"]
mod fixture;
mod gdk_keys;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, like `state.rs` and `view.rs`.
pub mod keys;
mod menus;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The tiles and the Open In list as plain data; `text` (JSON for QML) is
// Qt's only.
#[allow(
    dead_code,
    reason = "the file is shared whole; GTK reads the data, not its JSON text"
)]
#[path = "../../../wye-ui/src/picker/qml.rs"]
mod model;
mod panel;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, so its nested `mod tests;` is found in
// `state/tests.rs` (a symlink too).
pub mod state;
mod tile;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, so its nested `mod tests;` is found in
// `view/tests.rs`.
pub mod view;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use serde_json::Value;
use wye_api::actions::PickerAction;
use wye_api::picker::Placement;

use self::panel::{Built, Handlers};
use self::state::{Effect, PickerState};
use self::view::PickerView;
use crate::app::Presenter;
use crate::cli::Scheme;
use crate::overlay;
use crate::route::Action;
use crate::widgets::menu_icons;

/// The layer-shell namespace: compositor rules can match it (README).
pub const NAMESPACE: &str = "wye-picker";
/// The pointer must move this far before hovering selects (PICK-24).
const HOVER_SLACK: f64 = 3.0;
/// The panel's padding and border on each side (style.css), for the room a
/// row of tiles may take (PICK-13).
const PANEL_FRAME: i32 = 15;
/// The "⋯" column beside the tiles (PICK-08).
const MORE_COLUMN: i32 = 40;
/// The widest the tile rows may be without an output to measure.
const DEFAULT_ROOM: i32 = 1280;
/// How long a self-test waits for the first layout before opening a menu.
const SELF_TEST_MENU_DELAY: std::time::Duration = std::time::Duration::from_millis(150);

/// The picker surface: one window, reused for every request (PICK-27).
pub struct Picker {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Picker")
            .field("request_id", &self.inner.request_id.borrow())
            .finish_non_exhaustive()
    }
}

struct Inner {
    /// This picker, for work deferred to the main loop.
    me: Weak<Inner>,
    window: gtk::Window,
    /// The window's child: the whole output on layer shell.
    surface: gtk::Box,
    panel: gtk::Box,
    overflow: gtk::PopoverMenu,
    tile_menu: gtk::PopoverMenu,
    handlers: Rc<Handlers>,
    request_id: RefCell<String>,
    /// `None` once the request is answered (or before the first).
    state: RefCell<Option<PickerState>>,
    built: RefCell<Option<Built>>,
    was_active: Cell<bool>,
    hover_origin: Cell<Option<(f64, f64)>>,
    pointer_moved: Cell<bool>,
    /// `--self-test-child`: a request's `selfTest` key is read.
    self_test: bool,
}

impl Picker {
    pub fn new(app: &adw::Application, self_test: bool) -> Self {
        let inner = Rc::new_cyclic(|weak: &Weak<Inner>| Inner::new(app, weak, self_test));
        Inner::connect(&inner);
        Self { inner }
    }
}

impl Presenter for Picker {
    /// `ShowPicker(request_id, request)`: show, or replace the request
    /// shown (PICK-27).
    fn present(&self, key: &str, argument: &str) {
        self.inner.load(key, argument);
    }

    /// `ClosePicker(request_id)` closes it without an answer.
    fn act(&self, action: Action, key: &str, argument: &str) {
        match action {
            Action::Close => self.inner.dismiss(key),
            Action::Show | Action::Toggle => self.present(key, argument),
        }
    }

    /// PIPE-13: a shown request is answered before the host goes, so the
    /// service does not keep its link waiting.
    fn before_quit(&self, quit: Box<dyn FnOnce()>) {
        self.inner.quit(quit);
    }
}

fn handlers(weak: &Weak<Inner>) -> Handlers {
    let (hover, click, more) = (weak.clone(), weak.clone(), weak.clone());
    Handlers {
        hover: Box::new(move |index, x, y| {
            if let Some(inner) = hover.upgrade() {
                inner.hover(index, x, y);
            }
        }),
        click: Box::new(move |index, button, modifiers, x, y| {
            if let Some(inner) = click.upgrade() {
                inner.click(index, button, modifiers, x, y);
            }
        }),
        more: Box::new(move || {
            if let Some(inner) = more.upgrade() {
                inner.open_more();
            }
        }),
    }
}

impl Inner {
    fn new(app: &adw::Application, weak: &Weak<Self>, self_test: bool) -> Self {
        let window = overlay::window(app, NAMESPACE, "Choose a browser");
        window.set_hide_on_close(true);
        let surface = gtk::Box::builder().hexpand(true).vexpand(true).build();
        surface.add_css_class("wye-picker-surface");
        let panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .vexpand(true)
            .build();
        panel.add_css_class("wye-picker-panel");
        panel.set_accessible_role(gtk::AccessibleRole::Dialog);
        panel.update_property(&[gtk::accessible::Property::Label("Choose a browser")]);
        surface.append(&panel);
        window.set_child(Some(&surface));
        let popover = || {
            let menu = gtk::PopoverMenu::from_model(None::<&gio::MenuModel>);
            menu.set_has_arrow(false);
            menu.set_parent(&surface);
            menu
        };
        let overflow = popover();
        let tile_menu = popover();
        Self {
            me: weak.clone(),
            window,
            surface,
            panel,
            overflow,
            tile_menu,
            handlers: Rc::new(handlers(weak)),
            request_id: RefCell::default(),
            state: RefCell::default(),
            built: RefCell::default(),
            was_active: Cell::new(false),
            hover_origin: Cell::new(None),
            pointer_moved: Cell::new(false),
            self_test,
        }
    }

    /// Keys, focus, clicks outside the panel and the menus' actions.
    fn connect(this: &Rc<Self>) {
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
        for menu in [&this.overflow, &this.tile_menu] {
            let weak = Rc::downgrade(this);
            menu.connect_closed(move |_| {
                let Some(inner) = weak.upgrade() else { return };
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

    /// Show request `request_id` (a JSON `PickerRequest`), replacing the one
    /// shown (PICK-27).
    fn load(&self, request_id: &str, json: &str) {
        let view = match PickerView::parse(json) {
            Ok(view) => view,
            Err(error) => {
                // `ShowPicker` refuses what cannot be read; should one get
                // here anyway, answer it so the link is not left pending.
                tracing::warn!(%error, "cannot show the picker");
                answer::cancelled(request_id.to_owned());
                return;
            }
        };
        // PICK-27: a menu belongs to the request it was opened for.
        self.overflow.popdown();
        self.tile_menu.popdown();
        self.hover_origin.set(None);
        self.pointer_moved.set(false);
        let placement = view.placement.clone();
        let monitor = placement
            .as_ref()
            .and_then(|at| overlay::monitor_named(&at.output));
        let state = PickerState::new(view);
        let built = panel::build(&self.panel, &state, room(monitor.as_ref()), &self.handlers);
        request_id.clone_into(&mut self.request_id.borrow_mut());
        *self.built.borrow_mut() = Some(built);
        *self.state.borrow_mut() = Some(state);
        self.refresh();
        if !self.window.is_visible() {
            self.was_active.set(false);
        }
        self.place(placement.as_ref(), monitor.as_ref());
        // The link's token lets the compositor give the picker the focus
        // (PICK-23 needs it to cancel on focus loss); the request ID is
        // never one.
        if let Some(token) = activation_token(json) {
            self.window.set_startup_id(&token);
        }
        self.window.present();
        if let Some((at, monitor)) = placement.as_ref().zip(monitor.as_ref()) {
            // PICK-02 on X11, where the window is the panel's size.
            overlay::place_window(&self.window, (at.x, at.y), monitor, true);
        }
        if let Some(test) = self_test(json).filter(|_| self.self_test) {
            // After the first layout, so the menus find their anchors.
            let weak = self.me.clone();
            glib::timeout_add_local_once(SELF_TEST_MENU_DELAY, move || {
                if let Some(inner) = weak.upgrade() {
                    inner.open_test_menu(&test);
                }
            });
        }
    }

    /// PICK-02: on a layer surface, the panel centred on the pointer the
    /// service reported, on its output; otherwise in the middle.
    fn place(&self, placement: Option<&Placement>, monitor: Option<&gdk::Monitor>) {
        overlay::set_monitor(&self.window, monitor);
        tracing::debug!(
            ?placement,
            monitor = ?monitor.and_then(gdk::Monitor::connector),
            layer = overlay::is_layer(&self.window),
            "placing the picker"
        );
        let at = placement.zip(monitor.and_then(|m| overlay::output_size(Some(m))));
        let Some((at, (width, height))) = at.filter(|_| overlay::is_layer(&self.window)) else {
            self.panel.set_halign(gtk::Align::Center);
            self.panel.set_valign(gtk::Align::Center);
            self.panel.set_margin_start(0);
            self.panel.set_margin_top(0);
            return;
        };
        let panel_width = self.panel.measure(gtk::Orientation::Horizontal, -1).1;
        let panel_height = self
            .panel
            .measure(gtk::Orientation::Vertical, panel_width)
            .1;
        self.panel.set_halign(gtk::Align::Start);
        self.panel.set_valign(gtk::Align::Start);
        self.panel
            .set_margin_start(overlay::centred_on(at.x, panel_width, width));
        self.panel
            .set_margin_top(overlay::centred_on(at.y, panel_height, height));
    }

    /// The service closed request `request_id`: hide without answering.
    fn dismiss(&self, request_id: &str) {
        if *self.request_id.borrow() == request_id {
            self.finish();
        }
    }

    fn menu_open(&self) -> bool {
        self.overflow.is_visible() || self.tile_menu.is_visible()
    }

    fn key(
        &self,
        keyval: gdk::Key,
        keycode: u32,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        if self.menu_open() {
            return glib::Propagation::Proceed;
        }
        let press = gdk_keys::press(keyval, keycode, modifiers);
        // A modifier key changes only what is held (KEY-13).
        if press.is_modifier() {
            self.set_held(gdk_keys::after(keyval, modifiers, true));
            return glib::Propagation::Proceed;
        }
        let Some(state) = self.state.take() else {
            return glib::Propagation::Proceed;
        };
        let (state, effect) = state.key(&press);
        *self.state.borrow_mut() = Some(state);
        if self.apply(effect) {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    fn set_held(&self, modifiers: gdk::ModifierType) {
        let held = keys::modifiers(gdk_keys::modifier_bits(modifiers));
        self.update(|state| state.with_held(held));
    }

    /// PICK-22: hover selects, once the pointer has really moved (PICK-24:
    /// a panel opening under a resting pointer keeps the first tile).
    fn hover(&self, index: usize, x: f64, y: f64) {
        if !self.pointer_moved.get() {
            let Some((ox, oy)) = self.hover_origin.get() else {
                self.hover_origin.set(Some((x, y)));
                return;
            };
            if (x - ox).abs() < HOVER_SLACK && (y - oy).abs() < HOVER_SLACK {
                return;
            }
            self.pointer_moved.set(true);
        }
        self.update(|state| state.hover(index));
    }

    /// A click on a tile: the left button chooses, the middle one opens in
    /// the background, the right one opens the tile's menu (PICK-20,
    /// PICK-30, PICK-32, PICK-33).
    fn click(&self, index: usize, button: u32, modifiers: gdk::ModifierType, x: f64, y: f64) {
        if button == gdk::BUTTON_SECONDARY {
            self.open_tile_menu(index, x, y);
            return;
        }
        // A click tells what is held even when no key event did.
        self.set_held(modifiers);
        let effect = self.with_state(|state| state.activate(index, button == gdk::BUTTON_MIDDLE));
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// PICK-08, KEY-22: the "⋯" menu, under its button.
    fn open_more(&self) {
        let Some((state, icons)) = self
            .state
            .borrow()
            .as_ref()
            .map(|state| (menus::overflow(state), menus::open_in_icons(state)))
        else {
            return;
        };
        let button = self.built.borrow().as_ref().map(|built| built.more.clone());
        let bounds = button.and_then(|button| button.compute_bounds(&self.surface));
        self.overflow.set_menu_model(Some(&state));
        menu_icons::add(&self.overflow, &icons);
        if let Some(bounds) = bounds {
            self.overflow.set_pointing_to(Some(&rectangle(&bounds)));
        }
        self.overflow.set_position(gtk::PositionType::Bottom);
        self.overflow.popup();
    }

    /// PICK-30: tile `index`'s menu at (`x`, `y`) of the panel.
    fn open_tile_menu(&self, index: usize, x: f64, y: f64) {
        let Some(menu) = self.with_state(|state| menus::tile(state, index)) else {
            return;
        };
        let at = self
            .panel
            .compute_point(&self.surface, &point(x, y))
            .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
        self.tile_menu.set_menu_model(Some(&menu));
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a point inside the window fits an i32"
        )]
        let rect = gdk::Rectangle::new(at.0 as i32, at.1 as i32, 1, 1);
        self.tile_menu.set_pointing_to(Some(&rect));
        self.tile_menu.set_halign(gtk::Align::Start);
        self.tile_menu.set_position(gtk::PositionType::Bottom);
        self.tile_menu.popup();
    }

    /// `picker.open-in("<group>:<item>")` (PICK-28).
    fn open_in(&self, target: &str) {
        let effect = menus::pair(target).and_then(|(group, item)| {
            let item = item.parse().ok()?;
            self.with_state(|state| state.open_in(group, item))
        });
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// `picker.tile("<index>:<action>")` (PICK-30).
    fn tile_action(&self, target: &str) {
        let effect = menus::pair(target)
            .and_then(|(index, action)| self.with_state(|state| state.tile_action(index, action)));
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// Copy Link, Create Rule… (PICK-31) or Settings… from the "⋯" menu.
    fn overflow_action(&self, name: &str) {
        match name {
            menus::COPY_LINK => {
                self.apply(Effect::CopyLink);
            }
            menus::CREATE_RULE => {
                self.apply(Effect::CreateRule);
            }
            menus::SETTINGS => {
                if self.state.borrow().is_some() {
                    answer::cancelled(self.request_id.borrow().clone());
                    answer::open_settings();
                    self.finish();
                }
            }
            other => tracing::warn!(action = other, "unknown picker menu action"),
        }
    }

    /// Answer a shown request with `PickerCancelled`, then `quit`.
    fn quit(&self, quit: Box<dyn FnOnce()>) {
        if self.state.borrow().is_none() {
            quit();
            return;
        }
        let request_id = self.request_id.borrow().clone();
        self.finish();
        answer::cancelled_then(request_id, quit);
    }

    /// Close without opening the link (PICK-23).
    fn cancel(&self) {
        if self.state.borrow().is_none() {
            return;
        }
        answer::cancelled(self.request_id.borrow().clone());
        self.finish();
    }

    /// Carry out `effect`; true when the input was the picker's.
    fn apply(&self, effect: Effect) -> bool {
        let id = || self.request_id.borrow().clone();
        match effect {
            Effect::Ignored => return false,
            Effect::None => self.refresh(),
            Effect::Choose(choice) => {
                // PICK-29: the token comes from the input being handled, so
                // ask before the window goes.
                let token = answer::activation_token(&self.window);
                let request_id = id();
                self.finish();
                answer::chose(request_id, choice, token);
            }
            Effect::Cancel => self.cancel(),
            Effect::CopyLink => {
                answer::action(id(), PickerAction::CopyLink);
                self.finish();
            }
            Effect::CreateRule => {
                answer::action(id(), PickerAction::CreateRule);
                self.finish();
            }
            Effect::ShowMore => self.open_more(),
            Effect::MakePrimary(target) => answer::make_primary(target),
        }
        true
    }

    /// The request is answered or closed: forget it and hide.
    fn finish(&self) {
        *self.state.borrow_mut() = None;
        self.overflow.popdown();
        self.tile_menu.popdown();
        self.window.set_visible(false);
    }

    fn with_state<T>(&self, read: impl FnOnce(&PickerState) -> T) -> Option<T> {
        self.state.borrow().as_ref().map(read)
    }

    fn update(&self, change: impl FnOnce(PickerState) -> PickerState) {
        let state = self.state.take().map(change);
        *self.state.borrow_mut() = state;
        self.refresh();
    }

    /// Selection, dimming and the hint from the state (PICK-07, PICK-14).
    fn refresh(&self) {
        let state = self.state.borrow();
        let built = self.built.borrow();
        let (Some(state), Some(built)) = (state.as_ref(), built.as_ref()) else {
            return;
        };
        for ((index, tile), dimmed) in built.tiles.iter().enumerate().zip(state.dimmed()) {
            tile.set_selected(index == state.selected);
            tile.set_dimmed(dimmed);
        }
        built.hint.set_label(state.hint());
        built.hint.set_visible(!state.hint().is_empty());
    }
}

/// The room a row of tiles may take on `monitor` (PICK-13).
fn room(monitor: Option<&gdk::Monitor>) -> i32 {
    overlay::output_size(monitor).map_or(DEFAULT_ROOM, |(width, _)| {
        width - 2 * (overlay::EDGE_MARGIN + PANEL_FRAME) - MORE_COLUMN
    })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "graphene takes f32; a point inside a window fits"
)]
fn point(x: f64, y: f64) -> gtk::graphene::Point {
    gtk::graphene::Point::new(x as f32, y as f32)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "widget bounds inside a window fit an i32"
)]
fn rectangle(bounds: &gtk::graphene::Rect) -> gdk::Rectangle {
    gdk::Rectangle::new(
        bounds.x() as i32,
        bounds.y() as i32,
        bounds.width() as i32,
        bounds.height() as i32,
    )
}

/// The request's `activationToken` (PICK-01, LAUNCH-03), if it has one.
fn activation_token(json: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Focus {
        activation_token: Option<String>,
    }
    serde_json::from_str::<Focus>(json)
        .ok()?
        .activation_token
        .filter(|token| !token.is_empty())
}

/// Under `--self-test` a request may carry `selfTest: {scheme, menu}`
/// (fixtures/picker.json, the format crates/wye-ui reads): the colour scheme
/// to draw in (applied here), and the menu to open (`overflow`, `open-in`
/// for its Open In page, or `tile` for the first tile's), returned.
fn self_test(json: &str) -> Option<String> {
    let request = serde_json::from_str::<Value>(json).ok()?;
    let test = request.get("selfTest")?;
    if let Some(scheme) = test
        .get("scheme")
        .and_then(Value::as_str)
        .and_then(Scheme::parse)
    {
        crate::app::set_scheme(scheme);
    }
    test.get("menu").and_then(Value::as_str).map(str::to_owned)
}

impl Inner {
    /// The menu a self-test request asks for (see [`self_test`]).
    fn open_test_menu(&self, menu: &str) {
        match menu {
            "overflow" => self.open_more(),
            "open-in" => {
                self.open_more();
                if let Some(item) = menu_icons::item(&self.overflow, "Open In") {
                    item.activate();
                }
            }
            "tile" => {
                let centre = self.built.borrow().as_ref().and_then(|built| {
                    let tile = built.tiles.first()?;
                    let bounds = tile.root.compute_bounds(&self.panel)?;
                    Some((
                        f64::from(bounds.x() + bounds.width() / 2.0),
                        f64::from(bounds.y() + bounds.height() / 2.0),
                    ))
                });
                if let Some((x, y)) = centre {
                    self.open_tile_menu(0, x, y);
                }
            }
            other => tracing::warn!(menu = other, "unknown self-test menu"),
        }
    }
}
