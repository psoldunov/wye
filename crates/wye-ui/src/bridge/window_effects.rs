//! `WindowEffects`: the C++ shim for QML (design B). The picker (U05) uses
//! it for blur behind its popover (02-picker.md, "No blur available") and
//! for the activation token it hands the chosen browser (LAUNCH-03).
//!
//! ```qml
//! WindowEffects { id: effects; onActivationTokenReady: token => … }
//! Component.onCompleted: effects.blurBehind(window, true)
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

        /// Blur behind `window`. False when the compositor has no blur, or
        /// `window` is null: draw the background opaque instead.
        #[qinvokable]
        #[cxx_name = "blurBehind"]
        unsafe fn blur_behind(self: Pin<&mut Self>, window: *mut QWindow, enable: bool) -> bool;

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
    #[allow(
        clippy::unused_self,
        reason = "a Q_INVOKABLE is a method; the effect belongs to the window passed in"
    )]
    pub unsafe fn blur_behind(
        self: Pin<&mut Self>,
        window: *mut ffi::QWindow,
        enable: bool,
    ) -> bool {
        // SAFETY: the caller guarantees `window` is null or live; `as_mut`
        // turns null into `None`.
        let Some(window) = (unsafe { window.as_mut() }) else {
            tracing::warn!("blurBehind called without a window");
            return false;
        };
        // SAFETY: a QWindow is never moved by Qt once created.
        ffi::blur_behind(unsafe { Pin::new_unchecked(window) }, enable)
    }

    /// See the bridge declaration.
    ///
    /// # Safety
    ///
    /// As [`Self::blur_behind`].
    pub unsafe fn request_activation_token(
        self: Pin<&mut Self>,
        window: *mut ffi::QWindow,
        app_id: &QString,
    ) -> bool {
        // SAFETY: as in `blur_behind`.
        let Some(window) = (unsafe { window.as_mut() }) else {
            tracing::warn!("requestActivationToken called without a window");
            return false;
        };
        // SAFETY: a QWindow is never moved by Qt once created.
        let window = unsafe { Pin::new_unchecked(window) };
        ffi::request_activation_token(window, app_id, self.upcast_pin())
    }
}
