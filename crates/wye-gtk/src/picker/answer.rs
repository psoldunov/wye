//! The picker's answers to the service (PIPE-13, `docs/dbus-api.md`):
//! `PickerChose`, `PickerCancelled`, `PickerAction`, and the calls a menu
//! makes on the way (`SetPrimary`, `ShowWindow`). The picker is closed or
//! stays as it is whatever the answer; a failure is only logged.

use std::collections::HashMap;

use gtk::prelude::*;
use wye_api::actions::{PickerAction, Window};
use wye_api::context;
use zbus::zvariant::Value;

use super::state::ChoiceOut;
use crate::service;

fn log(what: &'static str) -> impl FnOnce(Result<(), wye_api::Error>) {
    move |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "{what} failed");
        }
    }
}

/// `PickerChose` for `request_id` with the xdg-activation `token` (empty
/// for none, PICK-29).
pub fn chose(request_id: String, choice: ChoiceOut, token: String) {
    service::request(
        move |proxy| async move {
            let mut options = HashMap::from([
                (context::OPTION_BACKGROUND, Value::from(choice.background)),
                (context::OPTION_NEW_WINDOW, Value::from(choice.new_window)),
            ]);
            if !token.is_empty() {
                options.insert(context::OPTION_ACTIVATION_TOKEN, Value::from(token));
            }
            proxy
                .picker_chose(&request_id, &choice.target, options)
                .await
        },
        log("PickerChose"),
    );
}

/// `PickerCancelled`: the link is not opened (PICK-23).
pub fn cancelled(request_id: String) {
    cancelled_then(request_id, || {});
}

/// `PickerCancelled`, then `then` once the service answered (or the call
/// failed): the host quits only after its answer went out.
pub fn cancelled_then(request_id: String, then: impl FnOnce() + 'static) {
    let logged = log("PickerCancelled");
    service::request(
        move |proxy| async move { proxy.picker_cancelled(&request_id).await },
        move |result| {
            logged(result);
            then();
        },
    );
}

/// `PickerAction`: Copy Link (KEY-22) or Create Rule… (PICK-31).
pub fn action(request_id: String, action: PickerAction) {
    service::request(
        move |proxy| async move { proxy.picker_action(&request_id, action.as_str()).await },
        log("PickerAction"),
    );
}

/// `SetPrimary` from a tile's menu (PICK-30); the picker stays open.
pub fn make_primary(target: String) {
    service::request(
        move |proxy| async move { proxy.set_primary(&target).await },
        log("SetPrimary"),
    );
}

/// Settings… from the "⋯" menu, through the service so the configured
/// frontend shows it (ADV-12).
pub fn open_settings() {
    service::request(
        |proxy| async move { proxy.show_window(Window::Settings.as_str(), "").await },
        log("ShowWindow"),
    );
}

/// An xdg-activation token from the input event being handled (PICK-29,
/// LAUNCH-03), so the compositor lets the browser take focus; empty when
/// there is none (see [`crate::links::activation_token`]).
pub fn activation_token(widget: &impl IsA<gtk::Widget>) -> String {
    crate::links::activation_token(&widget.as_ref().display()).unwrap_or_default()
}
