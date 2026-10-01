//! Opening a link from a window through Wye's own pipeline (BLK-17): links
//! in subtitles and callouts, the About dialog's website and issue tracker.
//! Wye is the default browser, so handing a link to the desktop would end up
//! here too; asking the service directly keeps the source and skips a hop.
//!
//! Counterpart of crates/wye-ui/src/about/link.rs.

use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{gdk, gio};
use wye_api::{Error, context};
use zbus::zvariant::Value;

use crate::service;

/// Ask the service to open `url`, with an activation token from the click
/// that asked for it, so the browser may take the focus (LAUNCH-03);
/// `on_error` runs on the main context with the failure, if there is one.
pub fn open(url: &str, on_error: impl FnOnce(Error) + 'static) {
    let url = url.to_owned();
    let token = gdk::Display::default().and_then(|display| activation_token(&display));
    service::request(
        move |proxy| async move {
            let mut link = HashMap::new();
            if let Some(token) = token {
                link.insert(context::ACTIVATION_TOKEN, Value::from(token));
            }
            proxy.open_link(&url, link).await
        },
        move |result| {
            if let Err(error) = result {
                on_error(error);
            }
        },
    );
}

/// An xdg-activation token for what `display`'s last input event started.
/// Only Wayland has them; `None` elsewhere or when the compositor gives
/// none. (The token is not tied to an app: the service hands it to whatever
/// it launches.)
pub fn activation_token(display: &gdk::Display) -> Option<String> {
    if !display.type_().name().contains("Wayland") {
        return None;
    }
    display
        .app_launch_context()
        .startup_notify_id(None::<&gio::AppInfo>, &[])
        .map(|token| token.to_string())
        .filter(|token| !token.is_empty())
}
