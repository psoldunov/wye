//! `OnboardingBackend`: the Rust side of the first-run window
//! (18-onboarding.md, ONB-01 to ONB-06). It keeps the step the user is on
//! and what the service said, hands QML one JSON view of both, and turns
//! each choice into a service call; the logic is in `crate::onboarding`.
//!
//! Used from `qml/onboarding/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, view_json, cxx_name = "viewJson")]
        #[qproperty(QString, error)]
        #[qproperty(QString, error_kind, cxx_name = "errorKind")]
        #[qproperty(bool, loaded)]
        #[qproperty(bool, offline)]
        #[qproperty(i32, step)]
        #[qproperty(i32, pending)]
        type OnboardingBackend = super::OnboardingBackendRust;

        /// Start over at the welcome step and read the service (ONB-01).
        #[qinvokable]
        fn restart(self: Pin<&mut Self>);

        /// The button after the current step (ONB-01 to ONB-05).
        #[qinvokable]
        fn next(self: Pin<&mut Self>);

        /// The Back button (ONB-06).
        #[qinvokable]
        fn back(self: Pin<&mut Self>);

        /// Move to step `index`, as a progress dot or the page row did.
        #[qinvokable]
        fn go(self: Pin<&mut Self>, index: i32);

        /// **Make Default** (ONB-02).
        #[qinvokable]
        #[cxx_name = "makeDefault"]
        fn make_default(self: Pin<&mut Self>);

        /// **Skip** on the default-browser step (ONB-02).
        #[qinvokable]
        #[cxx_name = "skipDefault"]
        fn skip_default(self: Pin<&mut Self>);

        /// The primary browser popup's choice, a target as JSON (ONB-03).
        #[qinvokable]
        #[cxx_name = "setPrimary"]
        fn set_primary(self: Pin<&mut Self>, target_json: &QString);

        /// A checkbox of the browsers list (ONB-03); `target_json` is the
        /// row's key.
        #[qinvokable]
        #[cxx_name = "toggleBrowser"]
        fn toggle_browser(self: Pin<&mut Self>, target_json: &QString, checked: bool);

        /// The **Launch at login** switch (ONB-04).
        #[qinvokable]
        #[cxx_name = "setLaunchAtLogin"]
        fn set_launch_at_login(self: Pin<&mut Self>, on: bool);

        /// End the walk-through: `onboardingDone` becomes true (ONB-06).
        /// Also what closing the window early does.
        #[qinvokable]
        fn finish(self: Pin<&mut Self>);

        /// Open a link through Wye's own pipeline (BLK-17, ONB-05).
        #[qinvokable]
        #[cxx_name = "openLink"]
        fn open_link(self: Pin<&mut Self>, url: &QString);

        /// Forget the last error.
        #[qinvokable]
        #[cxx_name = "clearError"]
        fn clear_error(self: Pin<&mut Self>);

        /// Show what a fixture says instead of asking the service, as the
        /// desktop `desktop` names (empty: this session's). False when the
        /// fixture is not valid.
        #[qinvokable]
        #[cxx_name = "loadFixture"]
        fn load_fixture(self: Pin<&mut Self>, fixture_json: &QString, desktop: &QString) -> bool;

        /// The walk-through ended: the window can close (ONB-05, ONB-06).
        #[qsignal]
        fn finished(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for OnboardingBackend {}
}

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::Value;
use wye_api::Error;

use crate::about::link;
use crate::error_text::{self, ErrorText};
use crate::onboarding::choices;
use crate::onboarding::desktop::Desktop;
use crate::onboarding::flow::{Effect, Flow};
use crate::onboarding::sync::{self, Action};
use crate::onboarding::view::View;
use crate::service;
use crate::settings::fixture::Fixture;
use crate::settings::snapshot::Snapshot;

/// The properties' values and the window's state.
pub struct OnboardingBackendRust {
    view_json: QString,
    error: QString,
    error_kind: QString,
    loaded: bool,
    offline: bool,
    step: i32,
    pending: i32,
    flow: Flow,
    snapshot: Snapshot,
    desktop: Desktop,
    finished: bool,
}

impl Default for OnboardingBackendRust {
    fn default() -> Self {
        Self {
            view_json: QString::default(),
            error: QString::default(),
            error_kind: QString::default(),
            loaded: false,
            offline: false,
            step: 0,
            pending: 0,
            flow: Flow::new(),
            snapshot: Snapshot::default(),
            desktop: Desktop::detect(),
            finished: false,
        }
    }
}

fn q(text: &str) -> QString {
    QString::from(text)
}

impl qobject::OnboardingBackend {
    /// Show `text` in the message bar; empty clears it.
    fn show_error(mut self: Pin<&mut Self>, text: &ErrorText) {
        self.as_mut().set_error_kind(QString::from(text.kind));
        self.set_error(QString::from(text.detail.as_str()));
    }

    /// Publish the step and the data for `snapshot`.
    fn show(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        self.as_mut().rust_mut().get_mut().snapshot = snapshot;
        self.publish();
    }

    fn publish(mut self: Pin<&mut Self>) {
        let rust = self.rust();
        let view = View::build(rust.flow, &rust.snapshot, rust.desktop);
        let index = i32::try_from(view.step_index).unwrap_or_default();
        let text = serde_json::to_string(&view).unwrap_or_else(|_| "{}".to_owned());
        self.as_mut().set_view_json(q(&text));
        self.as_mut().set_step(index);
        self.set_loaded(true);
    }

    fn fail(mut self: Pin<&mut Self>, error: &Error) {
        tracing::warn!(%error, "first-run request failed");
        self.as_mut().show_error(&error_text::describe(error));
    }

    fn add_pending(mut self: Pin<&mut Self>, delta: i32) {
        let next = (*self.pending() + delta).max(0);
        self.as_mut().set_pending(next);
    }

    /// Read everything from the service.
    fn reload(mut self: Pin<&mut Self>) {
        if *self.offline() {
            return;
        }
        self.as_mut().add_pending(1);
        service::request(
            self.qt_thread(),
            |proxy| async move { sync::load(&proxy).await },
            |mut backend, result| {
                backend.as_mut().add_pending(-1);
                backend.received(result);
            },
        );
    }

    /// An answer from the service. While another call is in flight the
    /// answer is dropped: it would undo the choice already shown.
    fn received(mut self: Pin<&mut Self>, result: Result<Snapshot, Error>) {
        if *self.offline() {
            // An answer that was on its way when a fixture took over.
            return;
        }
        match result {
            Ok(snapshot) if *self.pending() == 0 => {
                self.as_mut().show_error(&ErrorText::default());
                self.show(snapshot);
            }
            Ok(_) => {}
            Err(error) => {
                self.as_mut().fail(&error);
                // Show what the service holds, not the choice it refused.
                self.reload();
            }
        }
    }

    /// Carry out `action`, showing its effect at once.
    fn apply(mut self: Pin<&mut Self>, action: Action) {
        let offline = *self.offline();
        let before = self.rust().snapshot.clone();
        // A status the service has not confirmed is only shown when there
        // is no service to ask.
        let instant = offline || matches!(action, Action::Patch(_) | Action::Finish);
        if instant {
            self.as_mut().show(sync::preview(&before, &action));
        }
        if offline {
            return;
        }
        self.as_mut().add_pending(1);
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::run(&proxy, action, &before).await },
            |mut backend, result| {
                backend.as_mut().add_pending(-1);
                backend.received(result);
            },
        );
    }

    fn patch(self: Pin<&mut Self>, patch: Value) {
        self.apply(Action::Patch(patch));
    }

    /// What arriving at a step asks for.
    fn arrive(mut self: Pin<&mut Self>, effect: Effect) {
        match effect {
            Effect::None => {}
            Effect::SeedShownBrowsers => {
                let snapshot = self.rust().snapshot.clone();
                let foreign = choices::foreign_app_ids(&snapshot.services.services);
                if let Some(patch) =
                    choices::seed_patch(&snapshot.config, &snapshot.targets, &foreign)
                {
                    self.as_mut().patch(patch);
                }
            }
            Effect::Finish => self.finish(),
        }
    }

    fn move_to(mut self: Pin<&mut Self>, flow: Flow, effect: Effect) {
        self.as_mut().rust_mut().get_mut().flow = flow;
        self.as_mut().publish();
        self.arrive(effect);
    }

    /// See the bridge declaration.
    pub fn restart(mut self: Pin<&mut Self>) {
        {
            let rust = self.as_mut().rust_mut().get_mut();
            rust.flow = Flow::new();
            rust.finished = false;
        }
        self.as_mut().publish();
        self.reload();
    }

    /// See the bridge declaration.
    pub fn next(self: Pin<&mut Self>) {
        let (flow, effect) = self.rust().flow.next();
        self.move_to(flow, effect);
    }

    /// See the bridge declaration.
    pub fn back(self: Pin<&mut Self>) {
        let flow = self.rust().flow.back();
        self.move_to(flow, Effect::None);
    }

    /// See the bridge declaration.
    pub fn go(self: Pin<&mut Self>, index: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let (flow, effect) = self.rust().flow.at(index);
        self.move_to(flow, effect);
    }

    /// See the bridge declaration.
    pub fn make_default(self: Pin<&mut Self>) {
        self.apply(Action::MakeDefault);
    }

    /// See the bridge declaration.
    pub fn skip_default(mut self: Pin<&mut Self>) {
        // Only a default that is someone else's needs keeping.
        if !self.rust().snapshot.status.default_browser.is_default {
            self.as_mut().apply(Action::KeepCurrentDefault);
        }
        self.next();
    }

    /// See the bridge declaration.
    pub fn set_primary(mut self: Pin<&mut Self>, target_json: &QString) {
        match serde_json::from_str::<Value>(&target_json.to_string()) {
            Ok(target) => self.patch(choices::primary_patch(&target)),
            Err(error) => self
                .as_mut()
                .show_error(&ErrorText::plain(error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn toggle_browser(mut self: Pin<&mut Self>, target_json: &QString, checked: bool) {
        let target = match serde_json::from_str::<Value>(&target_json.to_string()) {
            Ok(target) => target,
            Err(error) => {
                self.as_mut()
                    .show_error(&ErrorText::plain(error.to_string()));
                return;
            }
        };
        let snapshot = self.rust().snapshot.clone();
        let foreign = choices::foreign_app_ids(&snapshot.services.services);
        let shown = choices::effective_shown(&snapshot.config, &snapshot.targets, &foreign);
        let next = choices::toggled(&shown, &target, checked);
        self.patch(choices::shown_patch(&next));
    }

    /// See the bridge declaration.
    pub fn set_launch_at_login(self: Pin<&mut Self>, on: bool) {
        // GEN-01: the Nix configuration owns login start; the key would
        // change nothing, so it is never written.
        if self.rust().snapshot.status.login_managed {
            return;
        }
        self.patch(choices::launch_patch(on));
    }

    /// See the bridge declaration.
    pub fn finish(mut self: Pin<&mut Self>) {
        if self.rust().finished {
            return;
        }
        self.as_mut().rust_mut().get_mut().finished = true;
        self.as_mut().apply(Action::Finish);
        self.finished();
    }

    /// See the bridge declaration.
    pub fn open_link(self: Pin<&mut Self>, url: &QString) {
        if *self.offline() {
            return;
        }
        link::open(self.qt_thread(), url.to_string(), Self::fail);
    }

    /// See the bridge declaration.
    pub fn clear_error(self: Pin<&mut Self>) {
        self.show_error(&ErrorText::default());
    }

    /// See the bridge declaration.
    pub fn load_fixture(
        mut self: Pin<&mut Self>,
        fixture_json: &QString,
        desktop: &QString,
    ) -> bool {
        let fixture = match Fixture::parse(&fixture_json.to_string()) {
            Ok(fixture) => fixture,
            Err(error) => {
                tracing::warn!(%error, "cannot read the first-run fixture");
                return false;
            }
        };
        let desktop = desktop.to_string();
        self.as_mut().set_offline(true);
        self.as_mut().show_error(&ErrorText::default());
        if !desktop.is_empty() {
            self.as_mut().rust_mut().get_mut().desktop = Desktop::of(&desktop);
        }
        let next = fixture.apply(&self.rust().snapshot);
        self.show(next);
        true
    }
}
