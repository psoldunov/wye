//! [`SettingsStore`]: what the Settings window shows and how it changes it
//! (03-settings-window.md). Every page, widget and sheet of one window reads
//! and writes through the window's store; nothing else talks to the service
//! about the configuration.
//!
//! It holds the configuration, status, targets and services the service
//! returned ([`Snapshot`]). Every control turns its value into a merge patch
//! ([`super::patch`]) and the store saves it with `UpdateConfig` (SET-06):
//! the change shows at once ([`SettingsStore::connect_changed`] fires), and a
//! failure reloads the truth and shows the error banner. A conflict reloads
//! the revision and applies the patch again ([`super::save`]). While the
//! window is on screen ([`SettingsStore::set_live`]) the store follows the
//! service's `PropertiesChanged` ([`crate::service::watch`]) and reads what
//! moved, so changes made elsewhere (`wye default`, a new browser) show
//! without a restart.
//!
//! Under `wye-gtk --self-test` there is no service:
//! [`SettingsStore::load_fixture`] fills the snapshot and every change stays
//! local (`offline`).
//!
//! Same behaviour as `SettingsBackend` in crates/wye-ui/src/bridge/settings.rs
//! and crates/wye-ui/src/settings/controls.rs; keep them in step.

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use serde_json::Value;
use wye_api::Error;

use super::fixture::Fixture;
use super::patch::{self, PatchError};
use super::save::{self, Change};
use super::snapshot::Snapshot;
use super::sync::{self, Action, Delta, Known};
use crate::error_text::{self, ErrorText};
use crate::service::{self, Change as ServiceChange};

/// The properties whose changes the window reads: `Status` (the default
/// browser, the file's health) and the two revisions `poll` compares.
const WATCHED: &[&str] = &["Status", "ConfigRevision", "InventoryRevision"];

mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::OnceLock;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::glib::subclass::Signal;

    use super::{ErrorText, Known, Snapshot, service};

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::SettingsStore)]
    pub struct SettingsStore {
        /// The service answered once (or a fixture was loaded).
        #[property(get)]
        pub(super) loaded: Cell<bool>,
        /// A fixture stands in for the service; changes stay local.
        #[property(get)]
        pub(super) offline: Cell<bool>,
        /// The configuration file can be written (`Status.config.writable`).
        #[property(get)]
        pub(super) writable: Cell<bool>,
        /// The window is on screen: announced changes are read at once.
        #[property(get)]
        pub(super) live: Cell<bool>,
        pub(super) snapshot: RefCell<Snapshot>,
        pub(super) known: Cell<Known>,
        /// Saves on their way: a configuration read now would undo them.
        pub(super) pending: Cell<u32>,
        pub(super) error: RefCell<ErrorText>,
        pub(super) connection_error: RefCell<ErrorText>,
        pub(super) watch: RefCell<Option<service::Subscription>>,
        /// The installed apps a fixture named (`GetApps(true)`), for the
        /// app chooser while `offline`.
        pub(super) fixture_apps: RefCell<Option<wye_api::apps::AppList>>,
        /// The global shortcuts a fixture named (`GetShortcuts`), for the
        /// Advanced page while `offline`.
        pub(super) fixture_shortcuts: RefCell<Option<wye_api::shortcuts::Shortcuts>>,
        /// The expansion catalogue a fixture named
        /// (`GetExpansionCatalogue`), for the URL expansion sheet.
        pub(super) fixture_expansion: RefCell<Option<wye_api::expansion::ExpansionCatalogue>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsStore {
        const NAME: &'static str = "WyeSettingsStore";
        type Type = super::SettingsStore;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SettingsStore {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    // The snapshot changed: read it again.
                    Signal::builder("changed").build(),
                    // The error or the connection error changed.
                    Signal::builder("messages-changed").build(),
                ]
            })
        }
    }
}

glib::wrapper! {
    /// The Settings window's data and the one way to change it. See the
    /// module docs.
    pub struct SettingsStore(ObjectSubclass<imp::SettingsStore>);
}

impl Default for SettingsStore {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SettingsStore {
    /// A store with nothing loaded yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ----- Reading -----------------------------------------------------

    /// A copy of everything the pages read.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.imp().snapshot.borrow().clone()
    }

    /// Read the snapshot without copying it. Do not change the store from
    /// `read`.
    pub fn with_snapshot<T>(&self, read: impl FnOnce(&Snapshot) -> T) -> T {
        read(&self.imp().snapshot.borrow())
    }

    /// The configuration value at the dotted `path`
    /// (`general.show-tray-icon`), if the file sets it.
    #[must_use]
    pub fn value(&self, path: &str) -> Option<Value> {
        let pointer = format!("/{}", path.replace('.', "/"));
        self.with_snapshot(|snapshot| snapshot.config.pointer(&pointer).cloned())
    }

    /// The boolean at `path`, or `default`.
    #[must_use]
    pub fn bool_value(&self, path: &str, default: bool) -> bool {
        self.value(path)
            .and_then(|value| value.as_bool())
            .unwrap_or(default)
    }

    /// The string at `path`, or `default`.
    #[must_use]
    pub fn string_value(&self, path: &str, default: &str) -> String {
        self.value(path)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| default.to_owned())
    }

    /// The last failed change, for the error banner; empty when there is
    /// none.
    #[must_use]
    pub fn error(&self) -> ErrorText {
        self.imp().error.borrow().clone()
    }

    /// Why the service cannot be read, for the warning banner; empty when it
    /// can.
    #[must_use]
    pub fn connection_error(&self) -> ErrorText {
        self.imp().connection_error.borrow().clone()
    }

    // ----- Following the service ----------------------------------------

    /// Run `changed` whenever the snapshot changes.
    pub fn connect_changed<F: Fn(&Self) + 'static>(&self, changed: F) -> glib::SignalHandlerId {
        self.connect_closure(
            "changed",
            false,
            glib::closure_local!(move |store: &Self| changed(store)),
        )
    }

    /// Run `changed` whenever the snapshot changes, for as long as `owner`
    /// lives: the handler goes with it. Rows that are built again (a list
    /// that changed) follow the store this way, so the rows they replace do
    /// not stay connected, with what they captured, for the window's life.
    pub fn connect_changed_while<F: Fn(&Self) + 'static>(
        &self,
        owner: &impl IsA<glib::Object>,
        changed: F,
    ) -> glib::SignalHandlerId {
        let owner = owner.upcast_ref::<glib::Object>();
        self.connect_closure(
            "changed",
            false,
            glib::closure_local!(
                #[watch]
                owner,
                move |store: &Self| {
                    // Only watched: the handler goes when `owner` does.
                    let _ = &owner;
                    changed(store);
                }
            ),
        )
    }

    /// Run `changed` whenever the error or the connection error changes.
    pub fn connect_messages_changed<F: Fn(&Self) + 'static>(
        &self,
        changed: F,
    ) -> glib::SignalHandlerId {
        self.connect_closure(
            "messages-changed",
            false,
            glib::closure_local!(move |store: &Self| changed(store)),
        )
    }

    /// Say whether the window is on screen. While it is, the changes the
    /// service announces are read at once; becoming visible reads everything.
    pub fn set_live(&self, live: bool) {
        if self.imp().live.replace(live) != live {
            self.notify_live();
        }
        if live {
            self.refresh();
        }
    }

    /// Read everything again from the service, and follow its changes.
    pub fn refresh(&self) {
        self.watch_service();
        self.reload(Known::default());
    }

    /// Read what changed since the last read (the window became active).
    pub fn poll(&self) {
        self.reload(self.imp().known.get());
    }

    /// Follow the properties the window shows, once (SET-06): no polling.
    fn watch_service(&self) {
        let imp = self.imp();
        if imp.offline.get() || imp.watch.borrow().is_some() {
            return;
        }
        let subscription = service::watch(
            WATCHED,
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |change| {
                    if change == ServiceChange::Restarted {
                        // A new service counts its revisions from 1 again.
                        store.imp().known.set(Known::default());
                    }
                    if store.live() {
                        store.poll();
                    }
                }
            ),
        );
        imp.watch.replace(Some(subscription));
    }

    fn reload(&self, known: Known) {
        if self.offline() {
            return;
        }
        service::request(
            move |proxy| async move { sync::poll(&proxy, known).await },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| store.received(result)
            ),
        );
    }

    /// A delta from the service. A configuration read while a save is in
    /// flight is dropped: it would undo the change already shown.
    fn received(&self, result: Result<Delta, Error>) {
        if self.offline() {
            // An answer that was on its way when a fixture took over.
            return;
        }
        let imp = self.imp();
        match result {
            Ok(delta) => {
                let delta = if imp.pending.get() > 0 {
                    Delta {
                        config: None,
                        ..delta
                    }
                } else {
                    delta
                };
                let applied = delta.apply(&imp.snapshot.borrow());
                match applied {
                    Ok(next) => {
                        imp.known.set(delta.known(imp.known.get()));
                        self.set_loaded(true);
                        // A poll that found nothing new must not rebuild every row.
                        if next != *imp.snapshot.borrow() {
                            self.show(next);
                        }
                        self.set_connection_error(ErrorText::default());
                    }
                    Err(error) => self.set_connection_error(error_text::describe(&error)),
                }
            }
            Err(error) => {
                tracing::debug!(%error, "cannot read the service");
                self.set_connection_error(error_text::describe(&error));
            }
        }
    }

    // ----- Changing ------------------------------------------------------

    /// Save a merge patch (SET-06): show it now, then send it.
    pub fn apply_patch(&self, patch: &Value) {
        let imp = self.imp();
        let before = imp.snapshot.borrow().clone();
        self.show(before.with_patch(patch, before.revision));
        if self.offline() {
            return;
        }
        imp.pending.set(imp.pending.get() + 1);
        // The configuration shown is the one the patch was built from,
        // including changes still on their way.
        let change = Change {
            patch: patch.clone(),
            base_revision: before.revision,
            base_config: before.config,
        };
        service::request(
            move |proxy| async move { save::save(&proxy, &change).await },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| store.saved(result)
            ),
        );
    }

    fn saved(&self, result: Result<(String, u64), Error>) {
        let imp = self.imp();
        imp.pending.set(imp.pending.get().saturating_sub(1));
        match result {
            // Saves run side by side: an answer that comes after a newer
            // one must not show the older configuration again; once the last
            // one is in, read what the service holds.
            Ok((_, revision)) if revision < imp.known.get().config => {
                if imp.pending.get() == 0 {
                    self.reload(Known::default());
                }
            }
            Ok((config, revision)) => {
                let current = imp.snapshot.borrow().clone();
                let next = if imp.pending.get() == 0 {
                    current.with_config(&config, revision).unwrap_or(current)
                } else {
                    Snapshot {
                        revision,
                        ..current
                    }
                };
                let known = imp.known.get();
                imp.known.set(Known {
                    config: revision,
                    ..known
                });
                self.show(next);
            }
            Err(error) => {
                self.fail(&error);
                // Show what the service really holds.
                self.reload(Known::default());
            }
        }
    }

    /// Save what a patch builder returned, or show why it failed.
    pub fn save_built(&self, built: Result<Value, PatchError>) {
        match built {
            Ok(patch) => self.apply_patch(&patch),
            Err(error) => self.set_error(ErrorText::plain(error.to_string())),
        }
    }

    /// Set the key at the dotted `path` to `value`: switches, radio groups,
    /// entries. `Value::Null` returns it to its default.
    pub fn set_value(&self, path: &str, value: Value) {
        self.save_built(patch::set(path, value));
    }

    /// Set the target at `path` (`browsers.primary`) to `target`.
    pub fn set_target(&self, path: &str, target: &Value) {
        self.save_built(patch::set_target(path, target));
    }

    /// Map web service `service` to `target` (APP-04, APP-06).
    pub fn set_service_target(&self, service: &str, target: &Value) {
        self.save_built(patch::set_service_target(service, target));
    }

    /// Set the modifier chooser at `path` to the pressed buttons (KEY-01).
    pub fn set_modifiers(&self, path: &str, names: &[String]) {
        self.save_built(patch::set_modifiers(path, names));
    }

    /// Hide callout `id` for good (BLK-09).
    pub fn dismiss_callout(&self, id: &str) {
        let dismissed = self.with_snapshot(|s| s.status.ui_state.dismissed_callouts.clone());
        self.update_ui_state(&patch::dismiss_callout(&dismissed, id));
    }

    /// Remember the page shown last (SET-08).
    pub fn remember_page(&self, page: &str) {
        self.update_ui_state(&patch::last_page(page));
    }

    /// Merge a patch into `Status.uiState` (`UpdateUiState`).
    pub fn update_ui_state(&self, patch: &Value) {
        let next = self.with_snapshot(|snapshot| snapshot.with_ui_state_patch(patch));
        self.show(next);
        if self.offline() {
            return;
        }
        let text = patch.to_string();
        service::request(
            move |proxy| async move { proxy.update_ui_state(&text).await },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| {
                    if let Err(error) = result {
                        store.fail(&error);
                    }
                }
            ),
        );
    }

    /// A button that acts on the machine (GEN-05, BRW-06, KEY-50), then read
    /// what it changed.
    pub fn act(&self, action: Action) {
        if self.offline() {
            return;
        }
        let known = self.imp().known.get();
        service::request(
            move |proxy| async move { sync::run(&proxy, action, known).await },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| match result {
                    Ok(Some(delta)) => store.received(Ok(delta)),
                    Ok(None) => {}
                    Err(error) => {
                        store.fail(&error);
                        store.poll();
                    }
                }
            ),
        );
    }

    /// Open a link through Wye's own pipeline (BLK-17).
    pub fn open_link(&self, url: &str) {
        if self.offline() {
            tracing::info!(%url, "a link was activated in the self-test");
            return;
        }
        crate::links::open(
            url,
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |error| store.fail(&error)
            ),
        );
    }

    /// Forget the last error.
    pub fn clear_error(&self) {
        self.set_error(ErrorText::default());
    }

    /// Show what a fixture says instead of asking the service; every change
    /// then stays local.
    ///
    /// # Errors
    ///
    /// When `fixture` is not service data; the store is unchanged then.
    pub fn load_fixture(&self, fixture: &Value) -> Result<(), serde_json::Error> {
        let fixture = Fixture::parse(&fixture.to_string())?;
        let imp = self.imp();
        if !imp.offline.replace(true) {
            self.notify_offline();
        }
        imp.watch.replace(None);
        self.set_error(ErrorText::default());
        self.set_connection_error(ErrorText::default());
        if fixture.apps.is_some() {
            imp.fixture_apps.replace(fixture.apps.clone());
        }
        if fixture.shortcuts.is_some() {
            imp.fixture_shortcuts.replace(fixture.shortcuts.clone());
        }
        if fixture.expansion.is_some() {
            imp.fixture_expansion.replace(fixture.expansion.clone());
        }
        let next = fixture.apply(&imp.snapshot.borrow());
        self.set_loaded(true);
        self.show(next);
        Ok(())
    }

    // ----- Internals -------------------------------------------------------

    /// Publish `snapshot`.
    fn show(&self, snapshot: Snapshot) {
        let imp = self.imp();
        let writable = snapshot.writable();
        imp.snapshot.replace(snapshot);
        if imp.writable.replace(writable) != writable {
            self.notify_writable();
        }
        self.emit_by_name::<()>("changed", &[]);
    }

    /// Mark the store loaded. What reads `loaded` (a callout, the status
    /// button) is shown again: `changed` fires too.
    fn set_loaded(&self, loaded: bool) {
        if self.imp().loaded.replace(loaded) != loaded {
            self.notify_loaded();
            self.emit_by_name::<()>("changed", &[]);
        }
    }

    fn fail(&self, error: &Error) {
        tracing::warn!(%error, "settings request failed");
        self.set_error(error_text::describe(error));
    }

    fn set_error(&self, text: ErrorText) {
        if *self.imp().error.borrow() != text {
            self.imp().error.replace(text);
            self.emit_by_name::<()>("messages-changed", &[]);
        }
    }

    fn set_connection_error(&self, text: ErrorText) {
        if *self.imp().connection_error.borrow() != text {
            self.imp().connection_error.replace(text);
            self.emit_by_name::<()>("messages-changed", &[]);
        }
    }
}

mod calls;
mod lists;
mod reading;

#[cfg(test)]
pub(crate) mod tests;
