//! `App`: the one object `qml/Main.qml` creates. It receives what the D-Bus
//! interfaces ask for (`crate::host`) as `routed` signals on the Qt thread,
//! and tells QML where each surface's root file is.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type App = super::AppRust;

        /// Start receiving routes; anything asked for before is delivered
        /// now, in order. False when another `App` is already attached.
        #[qinvokable]
        fn attach(self: Pin<&mut Self>) -> bool;

        /// The `qrc:` URL of a surface's root file, empty for an unknown
        /// surface name.
        #[qinvokable]
        #[cxx_name = "surfaceUrl"]
        fn surface_url(self: &Self, surface: &QString) -> QString;

        /// `--self-test --snapshots`: save every visible window as
        /// `<prefix>.png` (then `<prefix>-w2.png` …), print each file's path
        /// on stdout and return how many were saved.
        #[qinvokable]
        #[cxx_name = "saveSnapshots"]
        fn save_snapshots(self: &Self, prefix: &QString) -> i32;

        /// Deliver `action` to `surface`. `key` is the picker request id or
        /// the window name; `argument` is JSON or the window argument (see
        /// `crate::route`).
        #[qsignal]
        fn routed(
            self: Pin<&mut Self>,
            surface: QString,
            action: QString,
            key: QString,
            argument: QString,
        );

        /// `Windows1.Quit` was called.
        #[qsignal]
        #[cxx_name = "quitRequested"]
        fn quit_requested(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for App {}
}

use core::pin::Pin;
use std::io::Write as _;

use cxx_qt::{CxxQtThread, Threading as _};
use cxx_qt_lib::QString;

use crate::bridge::shim::ffi;
use crate::dispatch::{self, Sink, SinkClosed};
use crate::route::Delivery;
use crate::surface::Surface;

/// No state: routes arrive through [`AppSink`].
#[derive(Default)]
pub struct AppRust;

impl qobject::App {
    /// See the bridge declaration.
    pub fn attach(self: Pin<&mut Self>) -> bool {
        let sink = AppSink(self.qt_thread());
        match dispatch::global().attach(Box::new(sink)) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!("cannot attach the UI: {error}");
                false
            }
        }
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a Q_INVOKABLE is a method; the table it reads is static"
    )]
    pub fn surface_url(&self, surface: &QString) -> QString {
        Surface::from_name(&surface.to_string())
            .map(|surface| QString::from(surface.url().as_str()))
            .unwrap_or_default()
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a Q_INVOKABLE is a method; the windows are the application's"
    )]
    pub fn save_snapshots(&self, prefix: &QString) -> i32 {
        let saved = ffi::save_window_snapshots(prefix);
        let mut out = std::io::stdout().lock();
        for path in &saved {
            if let Err(error) = writeln!(out, "{path}") {
                tracing::warn!("cannot print a snapshot path: {error}");
            }
        }
        i32::try_from(saved.len()).unwrap_or(i32::MAX)
    }
}

/// Queues each delivery onto the Qt thread as a signal of the `App`.
struct AppSink(CxxQtThread<qobject::App>);

impl Sink for AppSink {
    fn deliver(&self, delivery: Delivery) -> Result<(), SinkClosed> {
        self.0
            .queue(move |app| emit(app, delivery))
            .map_err(|_| SinkClosed)
    }
}

fn emit(app: Pin<&mut qobject::App>, delivery: Delivery) {
    match delivery {
        Delivery::Route(route) => app.routed(
            QString::from(route.surface.name()),
            QString::from(route.action.as_str()),
            QString::from(route.key.as_str()),
            QString::from(route.argument.as_str()),
        ),
        Delivery::Quit => app.quit_requested(),
    }
}
