//! Opening a link from a window through Wye's own pipeline (BLK-17): the
//! About window's homepage and the first-run window's extension links.

use core::pin::Pin;
use std::collections::HashMap;

use cxx_qt::{CxxQtThread, Threading};
use wye_api::Error;

use crate::service;

/// Ask the service to open `url`; `on_error` runs on the Qt thread with the
/// failure, if there is one.
pub fn open<T>(thread: CxxQtThread<T>, url: String, on_error: fn(Pin<&mut T>, &Error))
where
    T: Threading + 'static,
{
    service::request(
        thread,
        move |proxy| async move { proxy.open_link(&url, HashMap::new()).await },
        move |object, result| {
            if let Err(error) = result {
                on_error(object, &error);
            }
        },
    );
}
