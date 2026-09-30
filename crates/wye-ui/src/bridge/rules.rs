//! `RulesBackend`: the Rust side of the Rules page and the rule editor
//! (08-rules.md). A QML singleton, so the page, its sheets and the Settings
//! window reach it as `RulesBackend`.
//!
//! The rules live in the configuration: the page reads them from
//! `SettingsBackend.configJson` and saves every change as the merge patch an
//! invokable here builds, through `SettingsBackend.applyPatch` (SET-06).
//! The logic is in [`crate::rules`]; this file converts types and calls the
//! service for the apps list and the rules file (RUL-02).
//!
//! Used from `qml/settings/RulesPage.qml` and `qml/rules/`.

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
        #[qproperty(QString, apps_json, cxx_name = "appsJson")]
        #[qproperty(QString, error)]
        #[qproperty(QString, notice)]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, busy)]
        type RulesBackend = super::RulesBackendRust;

        /// The Settings window was asked for the rule editor or the tester
        /// (`ShowWindow("rule-editor" | "test-rules", argument)`).
        #[qinvokable]
        fn request(self: Pin<&mut Self>, key: &QString, argument: &QString);

        /// The pending request as JSON `{key, argument}`, or empty; taking
        /// it clears it.
        #[qinvokable]
        #[cxx_name = "takeRequest"]
        fn take_request(self: Pin<&mut Self>) -> QString;

        /// Read the installed apps (`GetApps(true)`) into `appsJson`.
        #[qinvokable]
        #[cxx_name = "loadApps"]
        fn load_apps(self: Pin<&mut Self>);

        /// The list's rows (RUL-07) as JSON.
        #[qinvokable]
        fn rows(self: &Self, config_json: &QString, apps_json: &QString) -> QString;

        /// The patch that turns rule `index` on or off (RUL-07).
        #[qinvokable]
        #[cxx_name = "togglePatch"]
        fn toggle_patch(self: &Self, config_json: &QString, index: i32, enabled: bool) -> QString;

        /// The patch that moves rule `from` to `to` (RUL-04).
        #[qinvokable]
        #[cxx_name = "movePatch"]
        fn move_patch(self: &Self, config_json: &QString, from: i32, to: i32) -> QString;

        /// The patch that deletes rule `index`; Undo can bring it back
        /// (RUL-06).
        #[qinvokable]
        #[cxx_name = "deletePatch"]
        fn delete_patch(self: Pin<&mut Self>, config_json: &QString, index: i32) -> QString;

        /// The patch that deletes every rule; Undo can bring them back.
        #[qinvokable]
        #[cxx_name = "deleteAllPatch"]
        fn delete_all_patch(self: Pin<&mut Self>, config_json: &QString) -> QString;

        /// The patch that puts the last deleted rules back.
        #[qinvokable]
        #[cxx_name = "undoPatch"]
        fn undo_patch(self: Pin<&mut Self>, config_json: &QString) -> QString;

        /// Forget the undo (the toast timed out).
        #[qinvokable]
        #[cxx_name = "dropUndo"]
        fn drop_undo(self: Pin<&mut Self>);

        /// The patch that adds a copy of rule `index` below it.
        #[qinvokable]
        #[cxx_name = "duplicatePatch"]
        fn duplicate_patch(self: &Self, config_json: &QString, index: i32) -> QString;

        /// The patch that saves `draft` at `index` (-1: a new rule at the
        /// bottom, RUL-20); empty when the draft is not a rule.
        #[qinvokable]
        #[cxx_name = "savePatch"]
        fn save_patch(
            self: &Self,
            config_json: &QString,
            index: i32,
            draft_json: &QString,
        ) -> QString;

        /// A new rule's draft, filled from a `rule-editor` argument
        /// (PICK-31).
        #[qinvokable]
        #[cxx_name = "newDraft"]
        fn new_draft(self: &Self, config_json: &QString, argument: &QString) -> QString;

        /// Rule `index` as a draft, or empty.
        #[qinvokable]
        #[cxx_name = "draftFor"]
        fn draft_for(self: &Self, config_json: &QString, index: i32) -> QString;

        /// Whether the draft can be saved and what to flag (RUL-14, RUL-18,
        /// RUL-27), as JSON.
        #[qinvokable]
        fn check(self: &Self, draft_json: &QString, config_json: &QString) -> QString;

        /// The source-app rows of a draft (RUL-16) as JSON.
        #[qinvokable]
        #[cxx_name = "sourceRows"]
        fn source_rows(self: &Self, specs_json: &QString, apps_json: &QString) -> QString;

        /// The link the tester starts from for this draft (RUL-19).
        #[qinvokable]
        #[cxx_name = "testUrl"]
        fn test_url(self: &Self, draft_json: &QString) -> QString;

        /// Export every rule with its script to the chosen file (RUL-02).
        #[qinvokable]
        #[cxx_name = "exportTo"]
        fn export_to(self: Pin<&mut Self>, file_url: &QString);

        /// Add the rules of the chosen file at the end (RUL-02).
        #[qinvokable]
        #[cxx_name = "importFrom"]
        fn import_from(self: Pin<&mut Self>, file_url: &QString);

        /// A request is pending (`takeRequest`).
        #[qsignal]
        fn requested(self: Pin<&mut Self>);

        /// Rules were imported; the configuration changed.
        #[qsignal]
        fn imported(self: Pin<&mut Self>, count: i32);
    }

    impl cxx_qt::Threading for RulesBackend {}
}

use core::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use wye_core::{Modifiers, Rule};

use crate::rules::draft::{self, Prefill};
use crate::rules::ops::{self, Removed};
use crate::rules::{files, list};
use crate::service;

/// What the page keeps besides its properties.
#[derive(Default)]
pub struct RulesBackendRust {
    apps_json: QString,
    error: QString,
    notice: QString,
    can_undo: bool,
    busy: bool,
    /// `{key, argument}` of the last `request`, until taken.
    pending: Option<Value>,
    /// What the last delete took out (RUL-06).
    removed: Removed,
}

fn qs(text: &str) -> QString {
    QString::from(text)
}

fn text(value: &Value) -> QString {
    qs(&value.to_string())
}

fn index(value: i32) -> Option<usize> {
    usize::try_from(value).ok()
}

fn rules(config_json: &QString) -> Vec<Rule> {
    list::rules_of(&config_json.to_string())
}

/// `browsers.alternative-key` of the configuration (RUL-27).
fn alternative_key(config_json: &QString) -> Modifiers {
    serde_json::from_str::<Value>(&config_json.to_string())
        .ok()
        .and_then(|config| config.pointer("/browsers/alternative-key").cloned())
        .and_then(|keys| serde_json::from_value(keys).ok())
        .unwrap_or_else(|| wye_core::Config::default().browsers.alternative_key)
}

/// A seed for new rule IDs: the time in milliseconds.
fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| {
            u64::try_from(time.as_millis()).unwrap_or(u64::MAX)
        })
}

#[allow(
    clippy::unused_self,
    reason = "a QML invokable belongs to the object although the answer does not depend on its state"
)]
impl qobject::RulesBackend {
    /// See the bridge declaration.
    pub fn request(mut self: Pin<&mut Self>, key: &QString, argument: &QString) {
        let pending = json!({"key": key.to_string(), "argument": argument.to_string()});
        self.as_mut().rust_mut().get_mut().pending = Some(pending);
        self.requested();
    }

    /// See the bridge declaration.
    pub fn take_request(mut self: Pin<&mut Self>) -> QString {
        self.as_mut()
            .rust_mut()
            .get_mut()
            .pending
            .take()
            .map_or_else(QString::default, |pending| text(&pending))
    }

    /// See the bridge declaration.
    pub fn load_apps(self: Pin<&mut Self>) {
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.get_apps(true).await },
            |backend, answer| match answer {
                Ok(json) => backend.set_apps_json(qs(&json)),
                Err(error) => tracing::info!(%error, "no apps for the rule summaries"),
            },
        );
    }

    /// See the bridge declaration.
    pub fn rows(&self, config_json: &QString, apps_json: &QString) -> QString {
        let names = list::app_names(&apps_json.to_string());
        let rows = list::rows(&rules(config_json), &names);
        qs(&serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned()))
    }

    /// See the bridge declaration.
    pub fn toggle_patch(&self, config_json: &QString, at: i32, enabled: bool) -> QString {
        let rules = rules(config_json);
        index(at).map_or_else(QString::default, |at| {
            text(&ops::patch(&ops::toggled(&rules, at, enabled)))
        })
    }

    /// See the bridge declaration.
    pub fn move_patch(&self, config_json: &QString, from: i32, to: i32) -> QString {
        let rules = rules(config_json);
        match (index(from), index(to)) {
            (Some(from), Some(to)) => text(&ops::patch(&ops::moved(&rules, from, to))),
            _ => QString::default(),
        }
    }

    /// See the bridge declaration.
    pub fn delete_patch(self: Pin<&mut Self>, config_json: &QString, at: i32) -> QString {
        let Some(at) = index(at) else {
            return QString::default();
        };
        let (rest, removed) = ops::removed(&rules(config_json), at);
        let patch = text(&ops::patch(&rest));
        self.remember(removed);
        patch
    }

    /// See the bridge declaration.
    pub fn delete_all_patch(self: Pin<&mut Self>, config_json: &QString) -> QString {
        let removed = ops::all_removed(&rules(config_json));
        self.remember(removed);
        text(&ops::patch(&[]))
    }

    /// See the bridge declaration.
    pub fn undo_patch(mut self: Pin<&mut Self>, config_json: &QString) -> QString {
        let restored = self.removed.restored(&rules(config_json));
        self.as_mut().drop_undo();
        text(&ops::patch(&restored))
    }

    /// See the bridge declaration.
    pub fn drop_undo(self: Pin<&mut Self>) {
        self.remember(Removed::default());
    }

    fn remember(mut self: Pin<&mut Self>, removed: Removed) {
        self.as_mut().set_notice(qs(&removed.describe()));
        self.as_mut().set_can_undo(!removed.is_empty());
        self.rust_mut().get_mut().removed = removed;
    }

    /// See the bridge declaration.
    pub fn duplicate_patch(&self, config_json: &QString, at: i32) -> QString {
        let rules = rules(config_json);
        let id = draft::fresh_id(&rules, seed());
        index(at).map_or_else(QString::default, |at| {
            text(&ops::patch(&ops::duplicated(&rules, at, &id)))
        })
    }

    /// See the bridge declaration.
    pub fn save_patch(&self, config_json: &QString, at: i32, draft_json: &QString) -> QString {
        let rule = serde_json::from_str::<Value>(&draft_json.to_string())
            .map_err(|error| error.to_string())
            .and_then(|draft| draft::rule_of(&draft));
        match rule {
            Ok(rule) => text(&ops::patch(&ops::saved(
                &rules(config_json),
                index(at),
                rule,
            ))),
            Err(error) => {
                tracing::warn!(%error, "the rule draft is not a rule");
                QString::default()
            }
        }
    }

    /// See the bridge declaration.
    pub fn new_draft(&self, config_json: &QString, argument: &QString) -> QString {
        let id = draft::fresh_id(&rules(config_json), seed());
        text(&draft::new_draft(
            &Prefill::parse(&argument.to_string()),
            &id,
        ))
    }

    /// See the bridge declaration.
    pub fn draft_for(&self, config_json: &QString, at: i32) -> QString {
        let rules = rules(config_json);
        let id = draft::fresh_id(&rules, seed());
        index(at)
            .and_then(|at| draft::draft_for(&rules, at, &id))
            .map_or_else(QString::default, |draft| text(&draft))
    }

    /// See the bridge declaration.
    pub fn check(&self, draft_json: &QString, config_json: &QString) -> QString {
        let check = match serde_json::from_str::<Value>(&draft_json.to_string()) {
            Ok(draft) => draft::check(&draft, alternative_key(config_json)),
            Err(error) => draft::Check {
                error: error.to_string(),
                ..draft::Check::default()
            },
        };
        qs(&serde_json::to_string(&check).unwrap_or_default())
    }

    /// See the bridge declaration.
    pub fn source_rows(&self, specs_json: &QString, apps_json: &QString) -> QString {
        let specs: Vec<String> = serde_json::from_str(&specs_json.to_string()).unwrap_or_default();
        let rows = list::source_rows(&specs, &apps_json.to_string());
        qs(&serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned()))
    }

    /// See the bridge declaration.
    pub fn test_url(&self, draft_json: &QString) -> QString {
        serde_json::from_str::<Value>(&draft_json.to_string())
            .map_or_else(|_| QString::default(), |draft| qs(&draft::test_url(&draft)))
    }

    /// See the bridge declaration.
    pub fn export_to(mut self: Pin<&mut Self>, file_url: &QString) {
        let chosen = file_url.to_string();
        self.as_mut().set_busy(true);
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.export_rules().await },
            move |mut backend, answer| {
                backend.as_mut().set_busy(false);
                let written = answer
                    .map_err(|error| error.to_string())
                    .and_then(|text| files::write(&chosen, &text));
                match written {
                    Ok(()) => backend.set_notice(qs("Rules exported")),
                    Err(error) => backend.set_error(qs(&error)),
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn import_from(mut self: Pin<&mut Self>, file_url: &QString) {
        let text = match files::read(&file_url.to_string()) {
            Ok(text) => text,
            Err(error) => {
                self.set_error(qs(&error));
                return;
            }
        };
        self.as_mut().set_busy(true);
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.import_rules(&text).await },
            |mut backend, answer| {
                backend.as_mut().set_busy(false);
                match answer {
                    Ok(count) => {
                        let message = format!("Imported {count} rules");
                        backend.as_mut().set_notice(qs(&message));
                        backend.imported(i32::try_from(count).unwrap_or(i32::MAX));
                    }
                    Err(error) => backend.set_error(qs(&error.to_string())),
                }
            },
        );
    }
}
