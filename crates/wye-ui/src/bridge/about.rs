//! `AboutBackend`: the Rust side of the About window (17-dialogs.md,
//! DLG-ABT-01, DLG-ABT-02). It reads the service's `Version` and
//! `GetTroubleshooting`, and hands QML the `aboutData` of the page.
//!
//! It shows the pattern every backend uses to call the service:
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
        #[qproperty(QString, about_json, cxx_name = "aboutJson")]
        #[qproperty(QString, troubleshooting)]
        #[qproperty(QString, error)]
        #[qproperty(QString, error_kind, cxx_name = "errorKind")]
        #[qproperty(bool, offline)]
        type AboutBackend = super::AboutBackendRust;

        /// Read the service's `Version` into `serviceVersion` and its
        /// troubleshooting report into `troubleshooting`, or the failure
        /// into `error`.
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        /// Open a link through Wye's own pipeline (BLK-17).
        #[qinvokable]
        #[cxx_name = "openLink"]
        fn open_link(self: Pin<&mut Self>, url: &QString);

        /// Show what a fixture says instead of asking the service. False
        /// when the fixture is not valid.
        #[qinvokable]
        #[cxx_name = "loadFixture"]
        fn load_fixture(self: Pin<&mut Self>, fixture_json: &QString) -> bool;
    }

    impl cxx_qt::Threading for AboutBackend {}
}

use core::pin::Pin;

use cxx_qt::Threading as _;
use cxx_qt_lib::QString;
use wye_api::Error;

use crate::about::fixture::Fixture;
use crate::about::{info, link};
use crate::error_text::{self, ErrorText};
use crate::service;

/// The properties' values.
#[derive(Default)]
pub struct AboutBackendRust {
    service_version: QString,
    about_json: QString,
    troubleshooting: QString,
    error: QString,
    error_kind: QString,
    offline: bool,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

impl qobject::AboutBackend {
    /// Show `text` in the message bar; empty clears it.
    fn show_error(mut self: Pin<&mut Self>, text: &ErrorText) {
        self.as_mut().set_error_kind(QString::from(text.kind));
        self.set_error(QString::from(text.detail.as_str()));
    }

    /// Publish the version, and the page's `aboutData` for it.
    fn show_version(mut self: Pin<&mut Self>, version: &str) {
        let data = info::about_data(version).to_string();
        self.as_mut().set_about_json(q(&data));
        self.set_service_version(q(version));
    }

    fn fail(mut self: Pin<&mut Self>, error: &Error) {
        tracing::warn!(%error, "about request failed");
        self.as_mut().show_error(&error_text::describe(error));
    }

    /// See the bridge declaration.
    pub fn refresh(mut self: Pin<&mut Self>) {
        if *self.offline() {
            return;
        }
        // The page has a version to show before the service answers, and
        // when it never does.
        if self.service_version().is_empty() {
            self.as_mut().show_version(info::UI_VERSION);
        }
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.version().await.map_err(Error::from) },
            |mut backend, result| match result {
                Ok(version) => {
                    backend.as_mut().show_error(&ErrorText::default());
                    backend.show_version(&version);
                }
                Err(error) => backend.fail(&error),
            },
        );
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.get_troubleshooting().await },
            |mut backend, result| match result {
                Ok(text) => backend.as_mut().set_troubleshooting(q(&text)),
                Err(error) => {
                    backend.as_mut().set_troubleshooting(QString::default());
                    backend.fail(&error);
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn open_link(self: Pin<&mut Self>, url: &QString) {
        if *self.offline() {
            return;
        }
        link::open(self.qt_thread(), url.to_string(), Self::fail);
    }

    /// See the bridge declaration.
    pub fn load_fixture(mut self: Pin<&mut Self>, fixture_json: &QString) -> bool {
        Fixture::parse(&fixture_json.to_string()).map_or_else(
            |error| {
                tracing::warn!(%error, "cannot read the about fixture");
                false
            },
            |fixture| {
                self.as_mut().set_offline(true);
                self.as_mut().show_error(&ErrorText::default());
                self.as_mut()
                    .set_troubleshooting(q(&fixture.troubleshooting));
                self.show_version(&fixture.version);
                true
            },
        )
    }
}
