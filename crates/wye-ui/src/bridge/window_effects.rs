//! `WindowEffects`: the C++ shim for QML (design B). The picker (U05) uses
//! it for the activation token it hands the chosen browser (LAUNCH-03,
//! PICK-29). Blur behind the picker's panel is `PickerBackend.blurBehind`,
//! which blurs only the panel's rounded rectangle (PICK-01).
//!
//! ```qml
//! WindowEffects { id: effects; onActivationTokenReady: token => … }
//! effects.requestActivationToken(window, appId)
//! ```

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!(<QtGui/QWindow>);
        type QString = cxx_qt_lib::QString;
        type QWindow = crate::bridge::shim::ffi::QWindow;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        type WindowEffects = super::WindowEffectsRust;

        /// Ask for an xdg-activation token for `app_id` from the last input
        /// `window` received. True when asked: `activationTokenReady` follows
        /// (empty when refused). False when the session is not Wayland.
        #[qinvokable]
        #[cxx_name = "requestActivationToken"]
        unsafe fn request_activation_token(
            self: Pin<&mut Self>,
            window: *mut QWindow,
            app_id: &QString,
        ) -> bool;

        /// The token asked for with `requestActivationToken`; the shim emits
        /// it by name.
        #[qsignal]
        #[cxx_name = "activationTokenReady"]
        fn activation_token_ready(self: Pin<&mut Self>, token: QString);
    }
}

use core::pin::Pin;

use cxx_qt::casting::Upcast as _;
use cxx_qt_lib::QString;

use crate::bridge::shim::ffi;

/// No state: every call goes straight to the shim.
#[derive(Default)]
pub struct WindowEffectsRust;

impl qobject::WindowEffects {
    /// See the bridge declaration.
    ///
    /// # Safety
    ///
    /// `window` is null or points to a live `QWindow`; QML passes the
    /// window object it holds, which outlives the call.
    pub unsafe fn request_activation_token(
        self: Pin<&mut Self>,
        window: *mut ffi::QWindow,
        app_id: &QString,
    ) -> bool {
        // SAFETY: the caller guarantees `window` is null or live; `as_mut`
        // turns null into `None`.
        let Some(window) = (unsafe { window.as_mut() }) else {
            tracing::warn!("requestActivationToken called without a window");
            return false;
        };
        // SAFETY: a QWindow is never moved by Qt once created.
        let window = unsafe { Pin::new_unchecked(window) };
        ffi::request_activation_token(window, app_id, self.upcast_pin())
    }
}
