//! `AppChooserBackend`: the Rust side of the app chooser sheet
//! (17-dialogs.md, DLG-APP). The sheet (`qml/components/WyeAppChooser.qml`)
//! creates one; it reads `GetApps(true)`, filters it for the search
//! (DLG-APP-01) and lists the sections (DLG-APP-02) as JSON rows. The
//! choosing itself is QML's: `targetFor` and `browseTarget` turn a row or a
//! file into the target JSON (TGT-06, DLG-APP-04).
//!
//! Used from `qml/components/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, rows)]
        #[qproperty(QString, error)]
        #[qproperty(bool, loaded)]
        type AppChooserBackend = super::AppChooserBackendRust;

        /// Read the installed apps and the recent sources from the service.
        #[qinvokable]
        fn reload(self: Pin<&mut Self>);

        /// Use `appsJson` (a `GetApps` reply) instead of the service: the
        /// self-test's stand-in. False when it is not valid.
        #[qinvokable]
        #[cxx_name = "loadJson"]
        fn load_json(self: Pin<&mut Self>, apps_json: &QString) -> bool;

        /// List the rows for `search`; with `recent`, Recent Sources first
        /// (choosing source apps).
        #[qinvokable]
        fn filter(self: Pin<&mut Self>, search: &QString, recent: bool);

        /// The target the app `id` becomes when chosen (TGT-06), as JSON;
        /// empty for an unknown ID.
        #[qinvokable]
        #[cxx_name = "targetFor"]
        fn target_for(self: &Self, id: &QString) -> QString;

        /// The target for a file picked with "Browse…" (DLG-APP-04), as
        /// JSON; empty for a path that names nothing.
        #[qinvokable]
        #[cxx_name = "browseTarget"]
        fn browse_target(self: &Self, path: &QString) -> QString;

        /// The `TargetInfo` (JSON) the app `id` becomes when chosen, to name
        /// it before the service lists it (TGT-06); empty for an unknown ID.
        #[qinvokable]
        #[cxx_name = "chosenInfo"]
        fn chosen_info(self: &Self, id: &QString) -> QString;

        /// The same for a file picked with "Browse…" (DLG-APP-04); empty for
        /// a path that names nothing.
        #[qinvokable]
        #[cxx_name = "browseInfo"]
        fn browse_info(self: &Self, path: &QString) -> QString;
    }

    impl cxx_qt::Threading for AppChooserBackend {}
}

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use wye_api::apps::AppList;
use wye_api::targets::TargetInfo;

use crate::service;
use crate::settings::{chooser, snapshot};

/// The properties' values and the list being searched.
#[derive(Default)]
pub struct AppChooserBackendRust {
    rows: QString,
    error: QString,
    loaded: bool,
    list: AppList,
    search: String,
    recent: bool,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

fn info_json(info: &TargetInfo) -> QString {
    serde_json::to_string(info).map_or_else(|_| QString::default(), |text| q(&text))
}

impl qobject::AppChooserBackend {
    /// Show the rows of the current search.
    fn publish(mut self: Pin<&mut Self>) {
        let rust = self.rust();
        let rows = chooser::rows(&rust.list, &rust.search, rust.recent);
        let text = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned());
        self.as_mut().set_rows(q(&text));
    }

    fn use_list(mut self: Pin<&mut Self>, list: AppList) {
        self.as_mut().rust_mut().get_mut().list = list;
        self.as_mut().set_error(QString::default());
        self.as_mut().set_loaded(true);
        self.publish();
    }

    /// See the bridge declaration.
    pub fn reload(self: Pin<&mut Self>) {
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.get_apps(true).await },
            |mut backend, result| match result.and_then(|text| snapshot::decode_apps(&text)) {
                Ok(list) => backend.as_mut().use_list(list),
                Err(error) => {
                    tracing::warn!(%error, "cannot list the installed apps");
                    backend.as_mut().set_error(q(&error.to_string()));
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn load_json(mut self: Pin<&mut Self>, apps_json: &QString) -> bool {
        match snapshot::decode_apps(&apps_json.to_string()) {
            Ok(list) => {
                self.as_mut().use_list(list);
                true
            }
            Err(error) => {
                tracing::warn!(%error, "cannot read the apps fixture");
                false
            }
        }
    }

    /// See the bridge declaration.
    pub fn filter(mut self: Pin<&mut Self>, search: &QString, recent: bool) {
        let rust = self.as_mut().rust_mut().get_mut();
        rust.search = search.to_string();
        rust.recent = recent;
        self.publish();
    }

    /// See the bridge declaration.
    pub fn target_for(&self, id: &QString) -> QString {
        let id = id.to_string();
        self.rust()
            .list
            .apps
            .iter()
            .find(|app| app.id == id)
            .map(chooser::target_for)
            .map_or_else(QString::default, |target| q(&target.to_string()))
    }

    /// See the bridge declaration.
    pub fn chosen_info(&self, id: &QString) -> QString {
        let id = id.to_string();
        self.rust()
            .list
            .apps
            .iter()
            .find(|app| app.id == id)
            .map(chooser::chosen_info)
            .map_or_else(QString::default, |info| info_json(&info))
    }

    /// See the bridge declaration.
    pub fn browse_info(&self, path: &QString) -> QString {
        chooser::browse_info(&path.to_string(), &self.rust().list)
            .map_or_else(QString::default, |info| info_json(&info))
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn browse_target(&self, path: &QString) -> QString {
        chooser::browse_target(&path.to_string())
            .map_or_else(QString::default, |target| q(&target.to_string()))
    }
}
