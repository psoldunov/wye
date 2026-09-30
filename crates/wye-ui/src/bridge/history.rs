//! `HistoryBackend`: the Rust side of the History window (17-dialogs.md,
//! DLG-HIS-01 to DLG-HIS-04). It keeps what the service last said, hands
//! QML the rows for the current search, and turns the row actions into
//! service calls; the logic is in `crate::history`.
//!
//! Used from `qml/history/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, view_json, cxx_name = "viewJson")]
        #[qproperty(QString, error)]
        #[qproperty(bool, loaded)]
        #[qproperty(bool, offline)]
        type HistoryBackend = super::HistoryBackendRust;

        /// Read everything from the service.
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        /// Read what changed since the last read; the window calls it on a
        /// timer while it is visible (DLG-HIS-01, live refresh).
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);

        /// Narrow the rows to `query` (DLG-HIS-01).
        #[qinvokable]
        fn search(self: Pin<&mut Self>, query: &QString);

        /// Open entry `id` again: `how` is `picker` or `same-target`
        /// (DLG-HIS-03).
        #[qinvokable]
        fn reopen(self: Pin<&mut Self>, id: u64, how: &QString);

        /// Forget entry `id` (DLG-HIS-03).
        #[qinvokable]
        #[cxx_name = "deleteEntry"]
        fn delete_entry(self: Pin<&mut Self>, id: u64);

        /// Forget every entry (DLG-HIS-01).
        #[qinvokable]
        #[cxx_name = "clearHistory"]
        fn clear_history(self: Pin<&mut Self>);

        /// Switch history on (DLG-HIS-04).
        #[qinvokable]
        #[cxx_name = "turnOn"]
        fn turn_on(self: Pin<&mut Self>);

        /// Open the rule editor for entry `id`'s host and source app
        /// (DLG-HIS-03).
        #[qinvokable]
        #[cxx_name = "createRule"]
        fn create_rule(self: Pin<&mut Self>, id: u64);

        /// Forget the last error.
        #[qinvokable]
        #[cxx_name = "clearError"]
        fn clear_error(self: Pin<&mut Self>);

        /// Show what a fixture says instead of asking the service. False
        /// when the fixture is not valid.
        #[qinvokable]
        #[cxx_name = "loadFixture"]
        fn load_fixture(self: Pin<&mut Self>, fixture_json: &QString) -> bool;
    }

    impl cxx_qt::Threading for HistoryBackend {}
}

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::json;
use wye_api::Error;
use wye_api::actions::Reopen;

use crate::history::fixture::Fixture;
use crate::history::sync::{self, Action, Snapshot, Update};
use crate::history::view::{View, split_url};
use crate::service;
use crate::settings::sync::describe;

/// The properties' values and what the window shows.
#[derive(Default)]
pub struct HistoryBackendRust {
    view_json: QString,
    error: QString,
    loaded: bool,
    offline: bool,
    snapshot: Snapshot,
    query: String,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

impl qobject::HistoryBackend {
    /// Publish the rows of `snapshot` for the current search.
    fn show(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        let view = View::build(&snapshot.history, &snapshot.targets, &self.rust().query);
        let text = serde_json::to_string(&view).unwrap_or_else(|_| "{}".to_owned());
        self.as_mut().rust_mut().get_mut().snapshot = snapshot;
        self.as_mut().set_view_json(q(&text));
        self.set_loaded(true);
    }

    fn fail(mut self: Pin<&mut Self>, error: &Error) {
        tracing::warn!(%error, "history request failed");
        self.as_mut().set_error(q(&describe(error)));
    }

    /// An update from the service, applied to what the window shows.
    fn received(mut self: Pin<&mut Self>, result: Result<Option<Update>, Error>) {
        if *self.offline() {
            // An answer that was on its way when a fixture took over.
            return;
        }
        match result {
            Ok(Some(update)) => {
                let next = update.apply(&self.rust().snapshot);
                self.as_mut().set_error(QString::default());
                self.show(next);
            }
            Ok(None) => self.as_mut().set_error(QString::default()),
            Err(error) => self.fail(&error),
        }
    }

    fn reload(self: Pin<&mut Self>, known: sync::Known) {
        if *self.offline() {
            return;
        }
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::poll(&proxy, known).await },
            qobject::HistoryBackend::received,
        );
    }

    fn act(self: Pin<&mut Self>, action: Action) {
        if *self.offline() {
            return;
        }
        let known = self.rust().snapshot.known;
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::run(&proxy, action, known).await },
            qobject::HistoryBackend::received,
        );
    }

    /// See the bridge declaration.
    pub fn refresh(self: Pin<&mut Self>) {
        self.reload(sync::Known::default());
    }

    /// See the bridge declaration.
    pub fn poll(self: Pin<&mut Self>) {
        let known = self.rust().snapshot.known;
        self.reload(known);
    }

    /// See the bridge declaration.
    pub fn search(mut self: Pin<&mut Self>, query: &QString) {
        self.as_mut().rust_mut().get_mut().query = query.to_string();
        let snapshot = self.rust().snapshot.clone();
        self.show(snapshot);
    }

    /// See the bridge declaration.
    pub fn reopen(mut self: Pin<&mut Self>, id: u64, how: &QString) {
        match how.to_string().parse::<Reopen>() {
            Ok(how) => self.act(Action::Reopen(id, how)),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn delete_entry(self: Pin<&mut Self>, id: u64) {
        self.act(Action::Delete(id));
    }

    /// See the bridge declaration.
    pub fn clear_history(self: Pin<&mut Self>) {
        self.act(Action::Clear);
    }

    /// See the bridge declaration.
    pub fn turn_on(self: Pin<&mut Self>) {
        self.act(Action::TurnOn);
    }

    /// See the bridge declaration.
    pub fn create_rule(mut self: Pin<&mut Self>, id: u64) {
        let entry = self
            .rust()
            .snapshot
            .history
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned();
        let Some(entry) = entry else {
            self.as_mut().set_error(q("That entry is gone."));
            return;
        };
        // PICK-31: the prefill the picker's "Create Rule…" sends.
        let (host, _) = split_url(&entry.final_url);
        let prefill = json!({"domain": host, "sourceApp": entry.source}).to_string();
        self.act(Action::CreateRule { prefill });
    }

    /// See the bridge declaration.
    pub fn clear_error(self: Pin<&mut Self>) {
        self.set_error(QString::default());
    }

    /// See the bridge declaration.
    pub fn load_fixture(mut self: Pin<&mut Self>, fixture_json: &QString) -> bool {
        let fixture = match Fixture::parse(&fixture_json.to_string()) {
            Ok(fixture) => fixture,
            Err(error) => {
                tracing::warn!(%error, "cannot read the history fixture");
                return false;
            }
        };
        self.as_mut().set_offline(true);
        self.as_mut().set_error(QString::default());
        self.show(fixture.snapshot());
        true
    }
}
