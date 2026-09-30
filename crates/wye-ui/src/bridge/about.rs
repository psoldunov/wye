//! `AboutBackend`: the Rust side of the About window (17-dialogs.md,
//! DLG-ABT). U14 fills it.
//!
//! It already shows the pattern every backend uses to call the service:
//! [`crate::service::request`] runs the call on the D-Bus thread and queues
//! the result back onto the Qt thread, where it sets a property.
//!
//! Used from `qml/about/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, service_version, cxx_name = "serviceVersion")]
        #[qproperty(QString, error)]
        type AboutBackend = super::AboutBackendRust;

        /// Read the service's `Version` into `serviceVersion`, or the
        /// failure into `error`.
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for AboutBackend {}
}

use core::pin::Pin;

use cxx_qt::Threading as _;
use cxx_qt_lib::QString;

use crate::service;

/// The properties' values.
#[derive(Default)]
pub struct AboutBackendRust {
    service_version: QString,
    error: QString,
}

impl qobject::AboutBackend {
    /// See the bridge declaration.
    pub fn refresh(self: Pin<&mut Self>) {
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.version().await.map_err(wye_api::Error::from) },
            |backend, result| match result {
                Ok(version) => {
                    let mut backend = backend;
                    backend.as_mut().set_error(QString::default());
                    backend.set_service_version(QString::from(version.as_str()));
                }
                Err(error) => backend.set_error(QString::from(error.to_string().as_str())),
            },
        );
    }
}
