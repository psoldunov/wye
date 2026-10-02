//! The invokables of `SettingsBackend` that turn one control's value into
//! a change and save it (SET-06), and the queries the controls read: plain
//! values and targets (TGT-01 to TGT-07), the shown browsers sheet (SHOWN-01
//! to SHOWN-08), hotkeys and recorded keys (KEY-02, KEY-03, KEY-10), help
//! texts (BLK-08), callouts (BLK-09), the last page (SET-08) and the popup
//! count Escape reads (SET-07).
//!
//! They are declared in `bridge/settings.rs`; the bodies are here to keep
//! that file to the bridge, as in [`super::pages`].

use core::pin::Pin;

use cxx_qt::Threading as _;
use cxx_qt_lib::QString;
use serde_json::Value;
use wye_api::targets::TargetInfo;

use crate::bridge::settings::qobject::SettingsBackend;
use crate::picker::keys::QtKey;
use crate::service;
use crate::settings::shown::{self, Entry};
use crate::settings::{patch, record, view};

fn q(text: &str) -> QString {
    QString::from(text)
}

fn json_value(text: &QString) -> Result<Value, patch::PatchError> {
    patch::parse(&text.to_string())
}

impl SettingsBackend {
    fn save_shown(self: Pin<&mut Self>, change: impl FnOnce(&[Entry]) -> Vec<Entry>) {
        let next = change(&view::shown_entries(self.snapshot()));
        self.save(&shown::to_patch(&next));
    }

    /// See the bridge declaration.
    pub fn apply_patch(mut self: Pin<&mut Self>, patch_json: &QString) {
        match json_value(patch_json) {
            Ok(patch) => self.save(&patch),
            Err(error) => self.as_mut().show_message("", &error.to_string()),
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
            Err(error) => self.as_mut().show_message("", &error.to_string()),
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
            Err(error) => self.as_mut().show_message("", &error.to_string()),
        }
    }

    /// See the bridge declaration.
    pub fn shown_add(mut self: Pin<&mut Self>, target_json: &QString) {
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| shown::add(entries, &target)),
            Err(error) => self.as_mut().show_message("", &error.to_string()),
        }
    }

    /// See the bridge declaration.
    pub fn shown_remove(mut self: Pin<&mut Self>, target_json: &QString) {
        match json_value(target_json) {
            Ok(target) => self.save_shown(|entries| shown::remove(entries, &target)),
            Err(error) => self.as_mut().show_message("", &error.to_string()),
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
    pub fn remember_chosen(mut self: Pin<&mut Self>, info_json: &QString) {
        let text = info_json.to_string();
        if text.is_empty() {
            return;
        }
        match serde_json::from_str::<TargetInfo>(&text) {
            Ok(info) => {
                let next = self.snapshot().with_chosen(info);
                self.as_mut().show(next);
            }
            Err(error) => tracing::warn!(%error, "ignored an unreadable chosen app"),
        }
    }

    /// See the bridge declaration.
    pub fn remember_page(self: Pin<&mut Self>, page: &QString) {
        self.push_ui_state(&patch::last_page(&page.to_string()));
    }

    /// See the bridge declaration.
    pub fn update_ui_state(mut self: Pin<&mut Self>, patch_json: &QString) {
        match json_value(patch_json) {
            Ok(patch) => self.push_ui_state(&patch),
            Err(error) => self.as_mut().show_message("", &error.to_string()),
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
    pub fn popup_settling(self: Pin<&mut Self>) {
        let settling = self.settling().saturating_add(1);
        self.set_settling(settling);
    }

    /// See the bridge declaration.
    pub fn popup_settled(self: Pin<&mut Self>) {
        let settling = (*self.settling() - 1).max(0);
        self.set_settling(settling);
    }
}
