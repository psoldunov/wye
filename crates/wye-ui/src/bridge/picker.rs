//! `PickerBackend`: the picker's `QObject` (02-picker.md). It holds the
//! request the service sent, exposes what QML draws, turns input into the
//! pure transitions of [`crate::picker::state`], and sends the outcome to
//! the service (`PickerChose`, `PickerCancelled`, `PickerAction`, PIPE-13).
//!
//! Choosing asks QML for an xdg-activation token first (`tokenRequested`,
//! PICK-29); QML answers with `submit(token)`, empty when there is none.
//!
//! Used from `qml/picker/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("wye-ui/cpp/wye_shim.h");
        include!(<QtGui/QWindow>);
        type QString = cxx_qt_lib::QString;
        type QWindow = crate::bridge::shim::ffi::QWindow;

        /// Blur behind a rounded rectangle of `window`; false when the
        /// compositor cannot (PICK-01, "No blur available").
        #[namespace = "wye"]
        #[cxx_name = "blurBehindRect"]
        fn blur_behind_rect(
            window: Pin<&mut QWindow>,
            enable: bool,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            radius: f64,
        ) -> bool;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, request_id, cxx_name = "requestId")]
        #[qproperty(QString, tiles)]
        #[qproperty(QString, open_in, cxx_name = "openIn")]
        #[qproperty(i32, selected)]
        #[qproperty(QString, hint)]
        #[qproperty(i32, columns)]
        #[qproperty(i32, icon_size, cxx_name = "iconSize")]
        #[qproperty(i32, pitch)]
        #[qproperty(i32, badge_size, cxx_name = "badgeSize")]
        #[qproperty(bool, show_names, cxx_name = "showNames")]
        #[qproperty(bool, show_url, cxx_name = "showUrl")]
        #[qproperty(bool, preview)]
        #[qproperty(QString, url_host, cxx_name = "urlHost")]
        #[qproperty(QString, url_rest, cxx_name = "urlRest")]
        #[qproperty(QString, url_full, cxx_name = "urlFull")]
        #[qproperty(QString, source_name, cxx_name = "sourceName")]
        #[qproperty(QString, source_icon, cxx_name = "sourceIcon")]
        #[qproperty(bool, placed)]
        #[qproperty(QString, placement_output, cxx_name = "placementOutput")]
        #[qproperty(i32, placement_x, cxx_name = "placementX")]
        #[qproperty(i32, placement_y, cxx_name = "placementY")]
        type PickerBackend = super::PickerBackendRust;

        /// Show request `request_id` (a JSON `PickerRequest`), replacing the
        /// one shown (PICK-27). False when it cannot be read.
        #[qinvokable]
        fn load(self: Pin<&mut Self>, request_id: &QString, json: &QString) -> bool;

        /// The service closed request `request_id` (ClosePicker): forget it
        /// without answering. False when another request is shown.
        #[qinvokable]
        fn dismiss(self: Pin<&mut Self>, request_id: &QString) -> bool;

        /// A key press from QML's `Keys.onPressed`; true when handled.
        #[qinvokable]
        #[cxx_name = "keyPressed"]
        fn key_pressed(
            self: Pin<&mut Self>,
            key: i32,
            text: &QString,
            native_scan_code: i32,
            modifiers: i32,
        ) -> bool;

        /// The held modifiers changed (a modifier key was released).
        #[qinvokable]
        #[cxx_name = "modifiersChanged"]
        fn modifiers_changed(self: Pin<&mut Self>, modifiers: i32);

        /// The pointer is over tile `index` (PICK-22).
        #[qinvokable]
        fn hover(self: Pin<&mut Self>, index: i32);

        /// Tile `index` was clicked with `modifiers` held (the mouse event's
        /// `Qt::KeyboardModifiers`); `middle` for the middle button
        /// (PICK-20, PICK-32, PICK-33).
        #[qinvokable]
        fn activate(self: Pin<&mut Self>, index: i32, middle: bool, modifiers: i32);

        /// Open In entry `item` of group `group` (PICK-28).
        #[qinvokable]
        #[cxx_name = "openInTarget"]
        fn open_in_target(self: Pin<&mut Self>, group: i32, item: i32);

        /// Tile `index`'s context menu as JSON `[{action, label}]`; an empty
        /// action is a separator (PICK-30).
        #[qinvokable]
        #[cxx_name = "tileMenu"]
        fn tile_menu(self: &Self, index: i32) -> QString;

        /// A context-menu entry on tile `index` (PICK-30).
        #[qinvokable]
        #[cxx_name = "tileAction"]
        fn tile_action(self: Pin<&mut Self>, index: i32, action: &QString);

        /// A "⋯" menu action: `copy-link`, `create-rule` or `settings`
        /// (PICK-08).
        #[qinvokable]
        #[cxx_name = "overflowAction"]
        fn overflow_action(self: Pin<&mut Self>, action: &QString);

        /// Close without opening the link (PICK-23).
        #[qinvokable]
        fn cancel(self: Pin<&mut Self>);

        /// Send the pending choice with `token` (empty for none).
        #[qinvokable]
        fn submit(self: Pin<&mut Self>, token: &QString);

        /// Blur behind the panel at `x`, `y`, `width` × `height` with corner
        /// `radius`. False when not possible: draw the panel opaque.
        #[qinvokable]
        #[cxx_name = "blurBehind"]
        unsafe fn blur_behind(
            self: &Self,
            window: *mut QWindow,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            radius: f64,
        ) -> bool;

        /// Hide the picker; the request is answered.
        #[qsignal]
        #[cxx_name = "closeRequested"]
        fn close_requested(self: Pin<&mut Self>);

        /// Ask the compositor for an activation token for `app_id`, then
        /// call `submit` (PICK-29).
        #[qsignal]
        #[cxx_name = "tokenRequested"]
        fn token_requested(self: Pin<&mut Self>, app_id: QString);

        /// Open the "⋯" menu (KEY-22 "Show more targets").
        #[qsignal]
        #[cxx_name = "moreRequested"]
        fn more_requested(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for PickerBackend {}
}

use core::pin::Pin;
use std::collections::HashMap;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use wye_api::actions::{PickerAction, Window};
use wye_api::context;
use zbus::zvariant::Value;

use crate::bridge::shim::ffi::QWindow;
use crate::picker::keys::QtKey;
use crate::picker::qml;
use crate::picker::state::{ChoiceOut, Effect, PickerState};
use crate::picker::view::PickerView;
use crate::service;

/// The properties' values and the picker's state.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean Q_PROPERTY QML binds to"
)]
pub struct PickerBackendRust {
    request_id: QString,
    tiles: QString,
    open_in: QString,
    selected: i32,
    hint: QString,
    columns: i32,
    icon_size: i32,
    pitch: i32,
    badge_size: i32,
    show_names: bool,
    show_url: bool,
    preview: bool,
    url_host: QString,
    url_rest: QString,
    url_full: QString,
    source_name: QString,
    source_icon: QString,
    placed: bool,
    placement_output: QString,
    placement_x: i32,
    placement_y: i32,
    state: Option<PickerState>,
    /// A choice waiting for its activation token.
    choice: Option<ChoiceOut>,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

fn int(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn index(value: i32) -> Option<usize> {
    usize::try_from(value).ok()
}

fn bits(value: i32) -> u32 {
    u32::try_from(value).unwrap_or_default()
}

impl qobject::PickerBackend {
    /// See the bridge declaration.
    pub fn load(mut self: Pin<&mut Self>, request_id: &QString, json: &QString) -> bool {
        let view = match PickerView::parse(&json.to_string()) {
            Ok(view) => view,
            Err(error) => {
                tracing::warn!(%error, "cannot show the picker");
                return false;
            }
        };
        let metrics = view.metrics;
        self.as_mut().set_request_id(request_id.clone());
        self.as_mut().set_columns(int(view.columns()));
        self.as_mut().set_icon_size(i32::from(metrics.icon));
        self.as_mut().set_pitch(i32::from(metrics.pitch));
        self.as_mut().set_badge_size(i32::from(metrics.badge));
        self.as_mut().set_show_names(view.show_names);
        self.as_mut().set_show_url(view.show_url);
        self.as_mut().set_preview(view.preview);
        self.as_mut().set_url_host(q(&view.url.host));
        self.as_mut().set_url_rest(q(&view.url.rest));
        self.as_mut().set_url_full(q(&view.url.full));
        self.as_mut().set_source_name(q(&view.url.source_name));
        self.as_mut().set_source_icon(q(&view.url.source_icon));
        let placement = view.placement.clone();
        self.as_mut().set_placed(placement.is_some());
        let (output, x, y) = placement.map_or((String::new(), 0, 0), |p| (p.output, p.x, p.y));
        self.as_mut().set_placement_output(q(&output));
        self.as_mut().set_placement_x(x);
        self.as_mut().set_placement_y(y);
        let state = PickerState::new(view);
        self.as_mut()
            .set_open_in(q(&qml::text(&qml::open_in(&state))));
        let rust = self.as_mut().rust_mut().get_mut();
        rust.choice = None;
        rust.state = Some(state);
        self.refresh();
        true
    }

    /// See the bridge declaration.
    pub fn dismiss(mut self: Pin<&mut Self>, request_id: &QString) -> bool {
        if *self.request_id() != *request_id {
            return false;
        }
        self.as_mut().rust_mut().get_mut().state = None;
        self.close_requested();
        true
    }

    /// See the bridge declaration.
    pub fn key_pressed(
        mut self: Pin<&mut Self>,
        key: i32,
        text: &QString,
        native_scan_code: i32,
        modifiers: i32,
    ) -> bool {
        let press = QtKey {
            key: bits(key),
            text: text.to_string(),
            native_scancode: bits(native_scan_code),
            modifiers: bits(modifiers),
        };
        let Some(state) = self.as_mut().rust_mut().get_mut().state.take() else {
            return false;
        };
        let (state, effect) = state.key(&press);
        self.as_mut().rust_mut().get_mut().state = Some(state);
        self.apply(effect)
    }

    /// See the bridge declaration.
    pub fn modifiers_changed(self: Pin<&mut Self>, modifiers: i32) {
        let held = crate::picker::keys::modifiers(bits(modifiers));
        self.update(|state| state.with_held(held));
    }

    /// See the bridge declaration.
    pub fn hover(self: Pin<&mut Self>, index: i32) {
        if let Some(index) = self::index(index) {
            self.update(|state| state.hover(index));
        }
    }

    /// See the bridge declaration.
    pub fn activate(mut self: Pin<&mut Self>, index: i32, middle: bool, modifiers: i32) {
        // A click tells what is held even when no key event did.
        let held = crate::picker::keys::modifiers(bits(modifiers));
        self.as_mut().update(|state| state.with_held(held));
        let effect = self
            .with_state(|state| self::index(index).map(|i| state.activate(i, middle)))
            .flatten();
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// See the bridge declaration.
    pub fn open_in_target(self: Pin<&mut Self>, group: i32, item: i32) {
        let effect = self
            .with_state(|state| Some(state.open_in(index(group)?, index(item)?)))
            .flatten();
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// See the bridge declaration.
    pub fn tile_menu(&self, index: i32) -> QString {
        let entries = self
            .with_state(|state| self::index(index).map(|i| state.tile_menu(i)))
            .flatten()
            .unwrap_or_default();
        q(&qml::text(&entries))
    }

    /// See the bridge declaration.
    pub fn tile_action(self: Pin<&mut Self>, index: i32, action: &QString) {
        let action = action.to_string();
        let effect = self
            .with_state(|state| self::index(index).map(|i| state.tile_action(i, &action)))
            .flatten();
        if let Some(effect) = effect {
            self.apply(effect);
        }
    }

    /// See the bridge declaration.
    pub fn overflow_action(mut self: Pin<&mut Self>, action: &QString) {
        match action.to_string().as_str() {
            "copy-link" => {
                self.apply(Effect::CopyLink);
            }
            "create-rule" => {
                self.apply(Effect::CreateRule);
            }
            "settings" => {
                self.as_mut().answer_cancelled();
                service::request(
                    self.qt_thread(),
                    |proxy| async move { proxy.show_window(Window::Settings.as_str(), "").await },
                    |_, result| log("ShowWindow", result),
                );
                self.finish();
            }
            other => tracing::warn!(action = other, "unknown picker menu action"),
        }
    }

    /// See the bridge declaration.
    pub fn cancel(mut self: Pin<&mut Self>) {
        if self.state_ref().is_none() {
            return;
        }
        self.as_mut().answer_cancelled();
        self.finish();
    }

    /// See the bridge declaration.
    pub fn submit(mut self: Pin<&mut Self>, token: &QString) {
        let Some(choice) = self.as_mut().rust_mut().get_mut().choice.take() else {
            return;
        };
        let id = self.request_id().to_string();
        let token = token.to_string();
        service::request(
            self.qt_thread(),
            move |proxy| async move {
                let mut options = HashMap::from([
                    (context::OPTION_BACKGROUND, Value::from(choice.background)),
                    (context::OPTION_NEW_WINDOW, Value::from(choice.new_window)),
                ]);
                if !token.is_empty() {
                    options.insert(context::OPTION_ACTIVATION_TOKEN, Value::from(token));
                }
                proxy.picker_chose(&id, &choice.target, options).await
            },
            |_, result| log("PickerChose", result),
        );
        self.finish();
    }

    /// See the bridge declaration.
    ///
    /// # Safety
    ///
    /// `window` is null or points to a live `QWindow`; QML passes the
    /// picker's own window, which outlives the call.
    #[allow(
        clippy::unused_self,
        reason = "a Q_INVOKABLE is a method; the blur belongs to the window passed in"
    )]
    pub unsafe fn blur_behind(
        &self,
        window: *mut QWindow,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: f64,
    ) -> bool {
        // SAFETY: the caller guarantees `window` is null or live.
        let Some(window) = (unsafe { window.as_mut() }) else {
            return false;
        };
        // SAFETY: Qt never moves a QWindow once created.
        let window = unsafe { Pin::new_unchecked(window) };
        qobject::blur_behind_rect(window, true, x, y, width, height, radius)
    }

    fn state_ref(&self) -> Option<&PickerState> {
        self.rust().state.as_ref()
    }

    fn with_state<T>(&self, read: impl FnOnce(&PickerState) -> T) -> Option<T> {
        self.state_ref().map(read)
    }

    /// Replace the state with `change(state)` and refresh the properties.
    fn update(mut self: Pin<&mut Self>, change: impl FnOnce(PickerState) -> PickerState) {
        let rust = self.as_mut().rust_mut().get_mut();
        rust.state = rust.state.take().map(change);
        self.refresh();
    }

    /// Selection, hint and tiles (their dimming) from the state.
    fn refresh(mut self: Pin<&mut Self>) {
        let Some((selected, hint, tiles)) = self.with_state(|state| {
            (
                state.selected,
                state.hint().to_owned(),
                qml::text(&qml::tiles(state)),
            )
        }) else {
            return;
        };
        self.as_mut().set_selected(int(selected));
        self.as_mut().set_hint(q(&hint));
        self.set_tiles(q(&tiles));
    }

    /// Carry out `effect`; true when the input was the picker's.
    fn apply(mut self: Pin<&mut Self>, effect: Effect) -> bool {
        match effect {
            Effect::Ignored => return false,
            Effect::None => self.refresh(),
            Effect::Choose(choice) => {
                let app_id = q(&choice.app_id);
                self.as_mut().rust_mut().get_mut().choice = Some(choice);
                self.token_requested(app_id);
            }
            Effect::Cancel => self.cancel(),
            Effect::CopyLink => self.answer_action(PickerAction::CopyLink),
            Effect::CreateRule => self.answer_action(PickerAction::CreateRule),
            Effect::ShowMore => self.more_requested(),
            Effect::MakePrimary(target) => service::request(
                self.qt_thread(),
                |proxy| async move { proxy.set_primary(&target).await },
                |_, result| log("SetPrimary", result),
            ),
        }
        true
    }

    fn answer_cancelled(self: Pin<&mut Self>) {
        let id = self.request_id().to_string();
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.picker_cancelled(&id).await },
            |_, result| log("PickerCancelled", result),
        );
    }

    fn answer_action(self: Pin<&mut Self>, action: PickerAction) {
        let id = self.request_id().to_string();
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.picker_action(&id, action.as_str()).await },
            |_, result| log("PickerAction", result),
        );
        self.finish();
    }

    /// The request is answered: forget it and hide.
    fn finish(mut self: Pin<&mut Self>) {
        let rust = self.as_mut().rust_mut().get_mut();
        rust.state = None;
        rust.choice = None;
        self.close_requested();
    }
}

/// Log the outcome of a call to the service; the picker is already closed.
fn log(what: &str, result: Result<(), wye_api::Error>) {
    if let Err(error) = result {
        tracing::warn!(%error, "{what} failed");
    }
}
