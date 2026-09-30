//! `SettingsBackend`: the Rust side of the Settings window
//! (03-settings-window.md). A QML singleton, so every page, component and
//! sheet reaches it as `SettingsBackend`.
//!
//! It holds the configuration, status, targets and services the service
//! returned (`settings::snapshot`) and publishes them to QML as JSON text
//! properties. Every control calls one invokable that turns its value into
//! a merge patch (`settings::patch`) and saves it with `UpdateConfig`
//! (SET-06): the change shows at once, and a failure reloads the truth and
//! raises `error`. A conflict reloads the revision and applies the patch
//! again (`settings::sync`). A timer in QML calls `poll` so changes made
//! elsewhere (`wye default`, a new browser) show without a restart.
//!
//! Under `wye-ui --self-test` there is no service: `loadFixture` fills the
//! snapshot from the fixture and every change stays local (`offline`).
//!
//! `generation` counts every change of the snapshot: a QML binding that
//! calls a query invokable (`targetMenu`, `shownRows`) reads it first, so it
//! runs again when the data changed.
//!
//! Used from `qml/settings/` and `qml/components/`.

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
        #[qproperty(QString, config_json, cxx_name = "configJson")]
        #[qproperty(QString, status_json, cxx_name = "statusJson")]
        #[qproperty(QString, targets_json, cxx_name = "targetsJson")]
        #[qproperty(QString, services_json, cxx_name = "servicesJson")]
        #[qproperty(QString, fixture_apps_json, cxx_name = "fixtureAppsJson")]
        #[qproperty(QString, primary_name, cxx_name = "primaryName")]
        #[qproperty(QString, error)]
        #[qproperty(QString, connection_error, cxx_name = "connectionError")]
        #[qproperty(bool, loaded)]
        #[qproperty(bool, offline)]
        #[qproperty(bool, writable)]
        #[qproperty(bool, held_keys_available, cxx_name = "heldKeysAvailable")]
        #[qproperty(i32, pending)]
        #[qproperty(i32, generation)]
        #[qproperty(i32, popups)]
        #[qproperty(bool, clipboard_watch_available, cxx_name = "clipboardWatchAvailable")]
        #[qproperty(bool, expansion_loaded, cxx_name = "expansionLoaded")]
        #[qproperty(QString, shortcuts_json, cxx_name = "shortcutsJson")]
        type SettingsBackend = super::SettingsBackendRust;

        /// Read everything again from the service.
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        /// Read what changed since the last read (a timer calls this).
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);

        /// Forget the last error.
        #[qinvokable]
        #[cxx_name = "clearError"]
        fn clear_error(self: Pin<&mut Self>);

        /// Save a merge patch (SET-06). `patchJson` is the patch.
        #[qinvokable]
        #[cxx_name = "applyPatch"]
        fn apply_patch(self: Pin<&mut Self>, patch_json: &QString);

        /// Set the key at the dotted `path` (`general.show-tray-icon`) to
        /// `valueJson`: switches, radio groups, entries.
        #[qinvokable]
        #[cxx_name = "setValue"]
        fn set_value(self: Pin<&mut Self>, path: &QString, value_json: &QString);

        /// Set the target at `path` (`browsers.primary`) to `targetJson`.
        #[qinvokable]
        #[cxx_name = "setTarget"]
        fn set_target(self: Pin<&mut Self>, path: &QString, target_json: &QString);

        /// Map web service `service` to `targetJson` (APP-04, APP-06).
        #[qinvokable]
        #[cxx_name = "setServiceTarget"]
        fn set_service_target(self: Pin<&mut Self>, service: &QString, target_json: &QString);

        /// Set the modifier chooser at `path` to `namesJson`, a JSON array
        /// of the pressed buttons (KEY-01).
        #[qinvokable]
        #[cxx_name = "setModifiers"]
        fn set_modifiers(self: Pin<&mut Self>, path: &QString, names_json: &QString);

        /// The target menu's rows as JSON (TGT-02). `surface`: `browsers`,
        /// `apps`, `rule`. `service` is the mapped web service, if any.
        #[qinvokable]
        #[cxx_name = "targetMenu"]
        fn target_menu(
            self: &Self,
            surface: &QString,
            current_json: &QString,
            service: &QString,
        ) -> QString;

        /// The row for the closed popup (TGT-01) as JSON.
        #[qinvokable]
        #[cxx_name = "targetLabel"]
        fn target_label(
            self: &Self,
            surface: &QString,
            current_json: &QString,
            service: &QString,
        ) -> QString;

        /// The shown browsers sheet's rows as JSON (SHOWN-02, SHOWN-03).
        #[qinvokable]
        #[cxx_name = "shownRows"]
        fn shown_rows(self: &Self) -> QString;

        /// Check or uncheck a row (SHOWN-03).
        #[qinvokable]
        #[cxx_name = "shownToggle"]
        fn shown_toggle(self: Pin<&mut Self>, target_json: &QString, checked: bool);

        /// Move a checked row (SHOWN-03); indices are among checked rows.
        #[qinvokable]
        #[cxx_name = "shownMove"]
        fn shown_move(self: Pin<&mut Self>, from: i32, to: i32);

        /// Give a row a hotkey, or none when `key` is empty (SHOWN-04).
        #[qinvokable]
        #[cxx_name = "shownSetHotkey"]
        fn shown_set_hotkey(self: Pin<&mut Self>, target_json: &QString, key: &QString);

        /// Add an app chosen with "+" (SHOWN-05).
        #[qinvokable]
        #[cxx_name = "shownAdd"]
        fn shown_add(self: Pin<&mut Self>, target_json: &QString);

        /// Remove an app added with "+" (SHOWN-08).
        #[qinvokable]
        #[cxx_name = "shownRemove"]
        fn shown_remove(self: Pin<&mut Self>, target_json: &QString);

        /// The hotkey popup's choices as JSON (SHOWN-04, KEY-12).
        #[qinvokable]
        #[cxx_name = "hotkeyChoices"]
        fn hotkey_choices(self: &Self) -> QString;

        /// Check a recorded hotkey; JSON `{ok, key}` or `{ok, error}`.
        #[qinvokable]
        #[cxx_name = "checkHotkey"]
        fn check_hotkey(self: &Self, recorded: &QString) -> QString;

        /// A key press for a recorder (KEY-02): JSON `{kind: pending |
        /// cancel | clear | binding, stored, label}`.
        #[qinvokable]
        #[cxx_name = "recordKey"]
        fn record_key(
            self: &Self,
            key: i32,
            text: &QString,
            native_scan_code: i32,
            modifiers: i32,
            single: bool,
        ) -> QString;

        /// Stored bindings (a JSON array) as they read in a chip (KEY-03).
        #[qinvokable]
        #[cxx_name = "bindingLabels"]
        fn binding_labels(self: &Self, stored_json: &QString) -> QString;

        /// The help popover text for `id` (BLK-08); empty when unknown.
        #[qinvokable]
        #[cxx_name = "helpText"]
        fn help_text(self: &Self, id: &QString) -> QString;

        /// Whether the user dismissed callout `id` (BLK-09).
        #[qinvokable]
        #[cxx_name = "isCalloutDismissed"]
        fn is_callout_dismissed(self: &Self, id: &QString) -> bool;

        /// Hide callout `id` for good (BLK-09).
        #[qinvokable]
        #[cxx_name = "dismissCallout"]
        fn dismiss_callout(self: Pin<&mut Self>, id: &QString);

        /// Remember the page shown last (SET-08).
        #[qinvokable]
        #[cxx_name = "rememberPage"]
        fn remember_page(self: Pin<&mut Self>, page: &QString);

        /// Merge a patch into `Status.uiState` (`UpdateUiState`).
        #[qinvokable]
        #[cxx_name = "updateUiState"]
        fn update_ui_state(self: Pin<&mut Self>, patch_json: &QString);

        /// A button that acts on the machine: `make-default`,
        /// `stop-being-default`, `rescan`, `quit` (GEN-05, BRW-06, KEY-50).
        #[qinvokable]
        fn act(self: Pin<&mut Self>, action: &QString);

        /// Open a link through Wye's own pipeline (BLK-17).
        #[qinvokable]
        #[cxx_name = "openLink"]
        fn open_link(self: Pin<&mut Self>, url: &QString);

        /// A popup or sheet opened: while any is open, Escape closes it and
        /// not the window (SET-07). Components call this from `onOpened`.
        #[qinvokable]
        #[cxx_name = "popupOpened"]
        fn popup_opened(self: Pin<&mut Self>);

        /// A popup or sheet closed.
        #[qinvokable]
        #[cxx_name = "popupClosed"]
        fn popup_closed(self: Pin<&mut Self>);

        /// Ask the window to open a sheet by name (`shown-browsers`, and
        /// U11's rule editor and tester). The page that owns it listens to
        /// `sheetRequested`.
        #[qinvokable]
        #[cxx_name = "requestSheet"]
        fn request_sheet(self: Pin<&mut Self>, name: &QString);

        /// A sheet was asked for with `requestSheet`.
        #[qsignal]
        #[cxx_name = "sheetRequested"]
        fn sheet_requested(self: Pin<&mut Self>, name: QString);

        /// Ask the window to switch to a page (`picker`, `browsers`, …), for
        /// a link in a note (ADV-08). The window listens to `pageRequested`.
        #[qinvokable]
        #[cxx_name = "requestPage"]
        fn request_page(self: Pin<&mut Self>, name: &QString);

        /// A page was asked for with `requestPage`.
        #[qsignal]
        #[cxx_name = "pageRequested"]
        fn page_requested(self: Pin<&mut Self>, name: QString);

        /// Open a window in the UI host (`ShowWindow`): `script-editor`
        /// with a scope, `history`.
        #[qinvokable]
        #[cxx_name = "showWindow"]
        fn show_window(self: Pin<&mut Self>, window: &QString, argument: &QString);

        /// Open the script editor for `scope` (`global`, `rule:<id>`) when
        /// that script does not exist yet: turning a transform on with no
        /// script opens the editor (SCR-09). Does nothing when the service
        /// cannot say.
        #[qinvokable]
        #[cxx_name = "openScriptIfMissing"]
        fn open_script_if_missing(self: Pin<&mut Self>, scope: &QString);

        /// Show the picker with a sample link (PKS-06).
        #[qinvokable]
        #[cxx_name = "previewPicker"]
        fn preview_picker(self: Pin<&mut Self>);

        /// Forget every history entry (ADV-09).
        #[qinvokable]
        #[cxx_name = "clearHistory"]
        fn clear_history(self: Pin<&mut Self>);

        /// Set the section at the dotted `path` (`picker.keys`) to its
        /// defaults: "Reset to Defaults" (KEY-04).
        #[qinvokable]
        #[cxx_name = "resetSection"]
        fn reset_section(self: Pin<&mut Self>, path: &QString);

        /// May picker `action` take the recorded binding (KEY-21)? JSON
        /// `{status: free | duplicate | clash | invalid, message}`.
        #[qinvokable]
        #[cxx_name = "checkPickerKey"]
        fn check_picker_key(self: &Self, action: &QString, binding: &QString) -> QString;

        /// Give `action` the binding; with `replace`, take it from what
        /// used it (KEY-02, KEY-21).
        #[qinvokable]
        #[cxx_name = "setPickerKey"]
        fn set_picker_key(self: Pin<&mut Self>, action: &QString, binding: &QString, replace: bool);

        /// May the held-modifier action take these pressed names (JSON
        /// array)? Same answer as `checkPickerKey` (KEY-21).
        #[qinvokable]
        #[cxx_name = "checkPickerModifiers"]
        fn check_picker_modifiers(self: &Self, which: &QString, names_json: &QString) -> QString;

        /// Give the held-modifier action the pressed names (KEY-01, KEY-21).
        #[qinvokable]
        #[cxx_name = "setPickerModifiers"]
        fn set_picker_modifiers(
            self: Pin<&mut Self>,
            which: &QString,
            names_json: &QString,
            replace: bool,
        );

        /// Read the global shortcuts into `shortcutsJson` (ADV-05).
        #[qinvokable]
        #[cxx_name = "loadShortcuts"]
        fn load_shortcuts(self: Pin<&mut Self>);

        /// Bind a global shortcut; an empty binding clears it (ADV-05 to
        /// ADV-07).
        #[qinvokable]
        #[cxx_name = "setShortcut"]
        fn set_shortcut(self: Pin<&mut Self>, action: &QString, binding: &QString);

        /// Open the shortcut mechanism's own dialog (KEY-40 "Change…").
        #[qinvokable]
        #[cxx_name = "configureShortcuts"]
        fn configure_shortcuts(self: Pin<&mut Self>);

        /// Read the URL expansion catalogue (DLG-EXP); `expansionLoaded`
        /// turns true.
        #[qinvokable]
        #[cxx_name = "loadExpansion"]
        fn load_expansion(self: Pin<&mut Self>);

        /// The expansion sheet's rows as JSON (DLG-EXP-01, DLG-EXP-02).
        #[qinvokable]
        #[cxx_name = "expansionRows"]
        fn expansion_rows(self: &Self) -> QString;

        /// Turn a wrapper or a short-link domain on or off.
        #[qinvokable]
        #[cxx_name = "expansionToggle"]
        fn expansion_toggle(self: Pin<&mut Self>, id: &QString, enabled: bool);

        /// Add a short-link domain (DLG-EXP-02). Returns why it cannot be
        /// added, or an empty string.
        #[qinvokable]
        #[cxx_name = "expansionAddDomain"]
        fn expansion_add_domain(self: Pin<&mut Self>, domain: &QString) -> QString;

        /// Remove a domain the user added (DLG-EXP-02).
        #[qinvokable]
        #[cxx_name = "expansionRemoveDomain"]
        fn expansion_remove_domain(self: Pin<&mut Self>, domain: &QString);

        /// Show what a fixture says instead of asking the service; every
        /// change then stays local. False when the fixture is not valid.
        #[qinvokable]
        #[cxx_name = "loadFixture"]
        fn load_fixture(self: Pin<&mut Self>, fixture_json: &QString) -> bool;
    }

    impl cxx_qt::Threading for SettingsBackend {}
}

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::Value;
use wye_api::Error;
use wye_api::expansion::ExpansionCatalogue;
use wye_api::shortcuts::Shortcuts;

use crate::picker::keys::QtKey;
use crate::service;
use crate::settings::expansion::Catalogue;
use crate::settings::fixture::Fixture;
use crate::settings::shown::{self, Entry};
use crate::settings::snapshot::Snapshot;
use crate::settings::sync::{self, Action, Delta, Known};
use crate::settings::{patch, record, view};

/// The properties' values and the window's state.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean Q_PROPERTY QML binds to"
)]
pub struct SettingsBackendRust {
    config_json: QString,
    status_json: QString,
    targets_json: QString,
    services_json: QString,
    fixture_apps_json: QString,
    primary_name: QString,
    error: QString,
    connection_error: QString,
    loaded: bool,
    offline: bool,
    writable: bool,
    held_keys_available: bool,
    pending: i32,
    generation: i32,
    popups: i32,
    clipboard_watch_available: bool,
    expansion_loaded: bool,
    shortcuts_json: QString,
    pub(crate) snapshot: Snapshot,
    known: Known,
    /// What the service shipped for the expansion sheet (DLG-EXP).
    pub(crate) catalogue: Option<Catalogue>,
    /// Fixture data for the calls a fixture stands in for.
    pub(crate) fixture_shortcuts: Option<Shortcuts>,
    pub(crate) fixture_expansion: Option<ExpansionCatalogue>,
}

fn q(text: &str) -> QString {
    QString::from(text)
}

/// `value` as JSON text in a `QString`.
fn json<T: serde::Serialize>(value: &T) -> QString {
    q(&serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned()))
}

fn json_value(text: &QString) -> Result<Value, patch::PatchError> {
    patch::parse(&text.to_string())
}

impl qobject::SettingsBackend {
    pub(crate) fn snapshot(&self) -> &Snapshot {
        &self.rust().snapshot
    }

    /// Publish `snapshot` to the properties.
    pub(crate) fn show(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        let config = json(&snapshot.config);
        let status = json(&snapshot.status);
        let targets = json(&snapshot.targets);
        let services = json(&snapshot.services);
        let primary = q(&snapshot.primary_name());
        let (writable, held_keys) = (snapshot.writable(), snapshot.held_keys_available());
        let clipboard_watch = snapshot.clipboard_watch_available();
        self.as_mut().rust_mut().get_mut().snapshot = snapshot;
        self.as_mut().set_config_json(config);
        self.as_mut().set_status_json(status);
        self.as_mut().set_targets_json(targets);
        self.as_mut().set_services_json(services);
        self.as_mut().set_primary_name(primary);
        self.as_mut().set_writable(writable);
        self.as_mut().set_held_keys_available(held_keys);
        self.as_mut().set_clipboard_watch_available(clipboard_watch);
        let generation = self.generation().wrapping_add(1);
        self.set_generation(generation);
    }

    pub(crate) fn fail(mut self: Pin<&mut Self>, error: &Error) {
        tracing::warn!(%error, "settings request failed");
        self.as_mut().set_error(q(&sync::describe(error)));
    }

    fn add_pending(mut self: Pin<&mut Self>, delta: i32) {
        let next = (*self.pending() + delta).max(0);
        self.as_mut().set_pending(next);
    }

    /// Read `known` and later from the service.
    fn reload(self: Pin<&mut Self>, known: Known) {
        if *self.offline() {
            return;
        }
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::poll(&proxy, known).await },
            |mut backend, result| backend.as_mut().received(result),
        );
    }

    /// A delta from the service. A configuration read while a save is in
    /// flight is dropped: it would undo the change already shown.
    fn received(mut self: Pin<&mut Self>, result: Result<Delta, Error>) {
        if *self.offline() {
            // An answer that was on its way when a fixture took over.
            return;
        }
        match result {
            Ok(delta) => {
                let delta = if *self.pending() > 0 {
                    Delta {
                        config: None,
                        ..delta
                    }
                } else {
                    delta
                };
                let known = self.rust().known;
                match delta.apply(self.snapshot()) {
                    Ok(next) => {
                        self.as_mut().rust_mut().get_mut().known = delta.known(known);
                        // A poll that found nothing new must not restart every binding.
                        if next != *self.snapshot() {
                            self.as_mut().show(next);
                        }
                        self.as_mut().set_connection_error(QString::default());
                        self.as_mut().set_loaded(true);
                    }
                    Err(error) => self.as_mut().set_connection_error(q(&error.to_string())),
                }
            }
            Err(error) => {
                tracing::debug!(%error, "cannot read the service");
                self.as_mut()
                    .set_connection_error(q(&sync::describe(&error)));
            }
        }
    }

    /// Show `patch` now and save it (SET-06).
    pub(crate) fn save(mut self: Pin<&mut Self>, patch: &Value) {
        let before = self.snapshot().clone();
        let next = before.with_patch(patch, before.revision);
        self.as_mut().show(next);
        if *self.offline() {
            return;
        }
        self.as_mut().add_pending(1);
        let text = patch.to_string();
        let base = before.revision;
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::save(&proxy, &text, base).await },
            |mut backend, result| backend.as_mut().saved(result),
        );
    }

    fn saved(mut self: Pin<&mut Self>, result: Result<(String, u64), Error>) {
        self.as_mut().add_pending(-1);
        match result {
            Ok((config, revision)) => {
                let current = self.snapshot().clone();
                let next = if *self.pending() == 0 {
                    current.with_config(&config, revision).unwrap_or(current)
                } else {
                    Snapshot {
                        revision,
                        ..current
                    }
                };
                self.as_mut().rust_mut().get_mut().known.config = revision;
                self.show(next);
            }
            Err(error) => {
                self.as_mut().fail(&error);
                // Show what the service really holds.
                self.reload(Known::default());
            }
        }
    }

    /// Save the result of a pure builder, or show why it failed.
    pub(crate) fn save_built(mut self: Pin<&mut Self>, built: Result<Value, patch::PatchError>) {
        match built {
            Ok(patch) => self.save(&patch),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    fn save_shown(self: Pin<&mut Self>, change: impl FnOnce(&[Entry]) -> Vec<Entry>) {
        let next = change(&view::shown_entries(self.snapshot()));
        self.save(&shown::to_patch(&next));
    }

    /// See the bridge declaration.
    pub fn refresh(self: Pin<&mut Self>) {
        self.reload(Known::default());
    }

    /// See the bridge declaration.
    pub fn poll(self: Pin<&mut Self>) {
        let known = self.rust().known;
        self.reload(known);
    }

    /// See the bridge declaration.
    pub fn clear_error(self: Pin<&mut Self>) {
        self.set_error(QString::default());
    }

    /// See the bridge declaration.
    pub fn apply_patch(mut self: Pin<&mut Self>, patch_json: &QString) {
        match json_value(patch_json) {
            Ok(patch) => self.save(&patch),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn set_value(mut self: Pin<&mut Self>, path: &QString, value_json: &QString) {
        let built = json_value(value_json).and_then(|value| patch::set(&path.to_string(), value));
        self.as_mut().save_built(built);
    }

    /// See the bridge declaration.
    pub fn set_target(mut self: Pin<&mut Self>, path: &QString, target_json: &QString) {
        let built = json_value(target_json)
            .and_then(|target| patch::set_target(&path.to_string(), &target));
        self.as_mut().save_built(built);
    }

    /// See the bridge declaration.
    pub fn set_service_target(mut self: Pin<&mut Self>, service: &QString, target_json: &QString) {
        let built = json_value(target_json)
            .and_then(|target| patch::set_service_target(&service.to_string(), &target));
        self.as_mut().save_built(built);
    }

    /// See the bridge declaration.
    pub fn set_modifiers(mut self: Pin<&mut Self>, path: &QString, names_json: &QString) {
        let built = json_value(names_json).and_then(|names| {
            let names: Vec<String> = serde_json::from_value(names)
                .map_err(|error| patch::PatchError::Json(error.to_string()))?;
            patch::set_modifiers(&path.to_string(), &names)
        });
        self.as_mut().save_built(built);
    }

    /// See the bridge declaration.
    pub fn target_menu(
        &self,
        surface: &QString,
        current_json: &QString,
        service: &QString,
    ) -> QString {
        let current = json_value(current_json).unwrap_or(Value::Null);
        q(&view::target_menu(
            self.snapshot(),
            &surface.to_string(),
            &current,
            &service.to_string(),
        ))
    }

    /// See the bridge declaration.
    pub fn target_label(
        &self,
        surface: &QString,
        current_json: &QString,
        service: &QString,
    ) -> QString {
        let current = json_value(current_json).unwrap_or(Value::Null);
        q(&view::target_label(
            self.snapshot(),
            &surface.to_string(),
            &current,
            &service.to_string(),
        ))
    }

    /// See the bridge declaration.
    pub fn shown_rows(&self) -> QString {
        q(&view::shown_rows(self.snapshot()))
    }

    /// See the bridge declaration.
    pub fn shown_toggle(mut self: Pin<&mut Self>, target_json: &QString, checked: bool) {
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| shown::toggle(entries, &target, checked)),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn shown_move(self: Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        self.save_shown(|entries| shown::move_entry(entries, from, to));
    }

    /// See the bridge declaration.
    pub fn shown_set_hotkey(mut self: Pin<&mut Self>, target_json: &QString, key: &QString) {
        let key = key.to_string();
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| {
                shown::set_hotkey(
                    entries,
                    &target,
                    Some(key.as_str()).filter(|k| !k.is_empty()),
                )
            }),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn shown_add(mut self: Pin<&mut Self>, target_json: &QString) {
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| shown::add(entries, &target)),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn shown_remove(mut self: Pin<&mut Self>, target_json: &QString) {
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| shown::remove(entries, &target)),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// See the bridge declaration.
    pub fn hotkey_choices(&self) -> QString {
        q(&view::hotkey_choices(self.snapshot()))
    }

    /// See the bridge declaration.
    pub fn check_hotkey(&self, recorded: &QString) -> QString {
        q(&view::hotkey_check(self.snapshot(), &recorded.to_string()))
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn record_key(
        &self,
        key: i32,
        text: &QString,
        native_scan_code: i32,
        modifiers: i32,
        single: bool,
    ) -> QString {
        let press = QtKey {
            key: u32::try_from(key).unwrap_or_default(),
            text: text.to_string(),
            native_scancode: u32::try_from(native_scan_code).unwrap_or_default(),
            modifiers: u32::try_from(modifiers).unwrap_or_default(),
        };
        let recorded = record::record(&press, single);
        q(&serde_json::to_string(&recorded).unwrap_or_default())
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn binding_labels(&self, stored_json: &QString) -> QString {
        let stored: Vec<String> =
            serde_json::from_str(&stored_json.to_string()).unwrap_or_default();
        q(&serde_json::to_string(&record::labels(&stored)).unwrap_or_default())
    }

    /// See the bridge declaration.
    pub fn help_text(&self, id: &QString) -> QString {
        q(&view::help_text(self.snapshot(), &id.to_string()))
    }

    /// See the bridge declaration.
    pub fn is_callout_dismissed(&self, id: &QString) -> bool {
        self.snapshot().callout_dismissed(&id.to_string())
    }

    /// See the bridge declaration.
    pub fn dismiss_callout(self: Pin<&mut Self>, id: &QString) {
        let dismissed = self.snapshot().status.ui_state.dismissed_callouts.clone();
        self.push_ui_state(&patch::dismiss_callout(&dismissed, &id.to_string()));
    }

    /// See the bridge declaration.
    pub fn remember_page(self: Pin<&mut Self>, page: &QString) {
        self.push_ui_state(&patch::last_page(&page.to_string()));
    }

    /// See the bridge declaration.
    pub fn update_ui_state(mut self: Pin<&mut Self>, patch_json: &QString) {
        match json_value(patch_json) {
            Ok(patch) => self.push_ui_state(&patch),
            Err(error) => self.as_mut().set_error(q(&error.to_string())),
        }
    }

    /// Show a `UiState` change and send it.
    fn push_ui_state(mut self: Pin<&mut Self>, patch: &Value) {
        let next = self.snapshot().with_ui_state_patch(patch);
        self.as_mut().show(next);
        if *self.offline() {
            return;
        }
        let text = patch.to_string();
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.update_ui_state(&text).await },
            |mut backend, result| {
                if let Err(error) = result {
                    backend.as_mut().fail(&error);
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn act(mut self: Pin<&mut Self>, action: &QString) {
        let Some(action) = Action::parse(&action.to_string()) else {
            self.as_mut().set_error(q("Unknown action"));
            return;
        };
        if *self.offline() {
            return;
        }
        let known = self.rust().known;
        service::request(
            self.qt_thread(),
            move |proxy| async move { sync::run(&proxy, action, known).await },
            |mut backend, result| match result {
                Ok(Some(delta)) => backend.as_mut().received(Ok(delta)),
                Ok(None) => {}
                Err(error) => {
                    backend.as_mut().fail(&error);
                    backend.poll();
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn open_link(self: Pin<&mut Self>, url: &QString) {
        if *self.offline() {
            return;
        }
        let url = url.to_string();
        service::request(
            self.qt_thread(),
            move |proxy| async move {
                proxy
                    .open_link(&url, std::collections::HashMap::new())
                    .await
            },
            |mut backend, result| {
                if let Err(error) = result {
                    backend.as_mut().fail(&error);
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn request_sheet(self: Pin<&mut Self>, name: &QString) {
        self.sheet_requested(name.clone());
    }

    /// See the bridge declaration.
    pub fn popup_opened(self: Pin<&mut Self>) {
        let popups = self.popups().saturating_add(1);
        self.set_popups(popups);
    }

    /// See the bridge declaration.
    pub fn popup_closed(self: Pin<&mut Self>) {
        let popups = (*self.popups() - 1).max(0);
        self.set_popups(popups);
    }

    /// See the bridge declaration.
    pub fn load_fixture(mut self: Pin<&mut Self>, fixture_json: &QString) -> bool {
        let fixture = match Fixture::parse(&fixture_json.to_string()) {
            Ok(fixture) => fixture,
            Err(error) => {
                tracing::warn!(%error, "cannot read the settings fixture");
                return false;
            }
        };
        self.as_mut().set_offline(true);
        self.as_mut().set_error(QString::default());
        self.as_mut().set_connection_error(QString::default());
        if let Some(apps) = &fixture.apps {
            self.as_mut().set_fixture_apps_json(json(apps));
        }
        if fixture.shortcuts.is_some() {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .fixture_shortcuts
                .clone_from(&fixture.shortcuts);
        }
        if fixture.expansion.is_some() {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .fixture_expansion
                .clone_from(&fixture.expansion);
        }
        let next = fixture.apply(self.snapshot());
        self.as_mut().show(next);
        self.set_loaded(true);
        true
    }
}
