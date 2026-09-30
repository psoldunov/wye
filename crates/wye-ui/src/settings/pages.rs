//! The invokables of `SettingsBackend` that belong to the Picker, Extras
//! and Advanced pages and their sheets: the picker keys sheet (KEY-20 to
//! KEY-22), Reset to Defaults (KEY-04), Preview Picker (PKS-06), the global
//! shortcuts (ADV-05 to ADV-07), the URL expansion sheet (DLG-EXP), history
//! (ADV-09) and the windows the Advanced page opens (ADV-04).
//!
//! They are declared in `bridge/settings.rs`; the bodies are here to keep
//! that file to the bridge. Each one converts types and calls the plain
//! modules of this directory.

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::Value;
use wye_api::Error;
use wye_api::expansion::ExpansionCatalogue;
use wye_api::json;
use wye_api::shortcuts::Shortcuts;

use crate::bridge::settings::qobject::SettingsBackend;
use crate::service;
use crate::settings::expansion::{self, Catalogue};
use crate::settings::{patch, shortcuts, view};

fn q(text: &str) -> QString {
    QString::from(text)
}

/// The argument of `ShowWindow("script-editor", …)` for `scope`.
fn script_argument(scope: &str) -> String {
    serde_json::json!({ "scope": scope }).to_string()
}

fn names(names_json: &QString) -> Vec<String> {
    serde_json::from_str(&names_json.to_string()).unwrap_or_default()
}

/// The refusal a service call gives when the feature is not there (yet):
/// shown as the hint to bind the command by hand, not as an error (KEY-41).
const fn is_absent(error: &Error) -> bool {
    matches!(error, Error::Unavailable(_) | Error::NotImplemented(_))
}

impl SettingsBackend {
    /// See the bridge declaration.
    pub fn request_page(self: Pin<&mut Self>, name: &QString) {
        self.page_requested(name.clone());
    }

    /// Report a failed call the user asked for.
    fn report(mut self: Pin<&mut Self>, result: Result<(), Error>) {
        if let Err(error) = result {
            self.as_mut().fail(&error);
        }
    }

    /// See the bridge declaration.
    pub fn show_window(self: Pin<&mut Self>, window: &QString, argument: &QString) {
        if *self.offline() {
            return;
        }
        let (window, argument) = (window.to_string(), argument.to_string());
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.show_window(&window, &argument).await },
            |mut backend, result| backend.as_mut().report(result),
        );
    }

    /// See the bridge declaration.
    pub fn open_script_if_missing(self: Pin<&mut Self>, scope: &QString) {
        if *self.offline() {
            return;
        }
        let scope = scope.to_string();
        service::request(
            self.qt_thread(),
            {
                let scope = scope.clone();
                move |proxy| async move { proxy.script_exists(&scope).await }
            },
            move |backend, result| match result {
                Ok(false) => backend.show_window(&q("script-editor"), &q(&script_argument(&scope))),
                Ok(true) => {}
                // The service cannot say (not implemented yet): leave the editor closed.
                Err(error) => tracing::debug!(%error, "cannot tell whether the script exists"),
            },
        );
    }

    /// See the bridge declaration.
    pub fn preview_picker(self: Pin<&mut Self>) {
        if *self.offline() {
            return;
        }
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.preview_picker().await },
            |mut backend, result| backend.as_mut().report(result),
        );
    }

    /// See the bridge declaration.
    pub fn clear_history(self: Pin<&mut Self>) {
        if *self.offline() {
            return;
        }
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.clear_history().await },
            |mut backend, result| backend.as_mut().report(result),
        );
    }

    /// See the bridge declaration.
    pub fn reset_section(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        if *self.offline() {
            let built =
                patch::local_defaults(&path).and_then(|defaults| patch::set(&path, defaults));
            self.as_mut().save_built(built);
            return;
        }
        service::request(
            self.qt_thread(),
            {
                let path = path.clone();
                move |proxy| async move { proxy.get_defaults(&path).await }
            },
            move |mut backend, result| match result
                .and_then(|text| json::decode::<Value>("defaults", &text))
            {
                Ok(defaults) => {
                    let built = patch::set(&path, defaults);
                    backend.as_mut().save_built(built);
                }
                Err(error) => backend.as_mut().fail(&error),
            },
        );
    }

    /// See the bridge declaration.
    pub fn check_picker_key(&self, action: &QString, binding: &QString) -> QString {
        q(&view::picker_key_check(
            self.snapshot(),
            &action.to_string(),
            &binding.to_string(),
        ))
    }

    /// See the bridge declaration.
    pub fn set_picker_key(
        self: Pin<&mut Self>,
        action: &QString,
        binding: &QString,
        replace: bool,
    ) {
        let built = view::picker_key_patch(
            self.snapshot(),
            &action.to_string(),
            &binding.to_string(),
            replace,
        );
        match built {
            Ok(patch) => self.save(&patch),
            Err(message) => self.show_message("", &message),
        }
    }

    /// See the bridge declaration.
    pub fn check_picker_modifiers(&self, which: &QString, names_json: &QString) -> QString {
        q(&view::modifiers_check(
            self.snapshot(),
            &which.to_string(),
            &names(names_json),
        ))
    }

    /// See the bridge declaration.
    pub fn set_picker_modifiers(
        self: Pin<&mut Self>,
        which: &QString,
        names_json: &QString,
        replace: bool,
    ) {
        let built = view::modifiers_patch(
            self.snapshot(),
            &which.to_string(),
            &names(names_json),
            replace,
        );
        match built {
            Ok(patch) => self.save(&patch),
            Err(message) => self.show_message("", &message),
        }
    }

    fn show_shortcuts(mut self: Pin<&mut Self>, wire: Option<&Shortcuts>) {
        let config = self.snapshot().config.clone();
        let view = wire.map_or_else(
            || shortcuts::fallback(&config),
            |wire| shortcuts::from_wire(wire, &config),
        );
        let text = serde_json::to_string(&view).unwrap_or_default();
        self.as_mut().set_shortcuts_json(q(&text));
    }

    /// See the bridge declaration.
    pub fn load_shortcuts(self: Pin<&mut Self>) {
        if *self.offline() {
            let wire = self.rust().fixture_shortcuts.clone();
            self.show_shortcuts(wire.as_ref());
            return;
        }
        service::request(
            self.qt_thread(),
            |proxy| async move { json::decode::<Shortcuts>("shortcuts", &proxy.get_shortcuts().await?) },
            |mut backend, result| match result {
                Ok(wire) => backend.as_mut().show_shortcuts(Some(&wire)),
                Err(error) => {
                    tracing::debug!(%error, "no global shortcuts to list");
                    backend.as_mut().show_shortcuts(None);
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn set_shortcut(mut self: Pin<&mut Self>, action: &QString, binding: &QString) {
        let (action, binding) = (action.to_string(), binding.to_string());
        if *self.offline() {
            let value = if binding.is_empty() {
                Value::Null
            } else {
                Value::from(binding)
            };
            let built = patch::set(&format!("shortcuts.{action}"), value);
            self.as_mut().save_built(built);
            self.load_shortcuts();
            return;
        }
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.set_shortcut(&action, &binding).await },
            |mut backend, result| {
                // Without a mechanism the row shows the command to bind by hand (KEY-41).
                if let Err(error) = result
                    && !is_absent(&error)
                {
                    backend.as_mut().fail(&error);
                }
                backend.load_shortcuts();
            },
        );
    }

    /// See the bridge declaration.
    pub fn configure_shortcuts(self: Pin<&mut Self>) {
        if *self.offline() {
            return;
        }
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.configure_shortcuts().await },
            |mut backend, result| backend.as_mut().report(result),
        );
    }

    fn use_catalogue(mut self: Pin<&mut Self>, wire: &ExpansionCatalogue) {
        let custom = self
            .snapshot()
            .typed_config()
            .advanced
            .expansion
            .custom_short_links;
        self.as_mut().rust_mut().get_mut().catalogue = Some(Catalogue::from_wire(wire, &custom));
        self.as_mut().set_expansion_loaded(true);
        // Bindings that show the rows read the generation.
        let generation = self.generation().wrapping_add(1);
        self.set_generation(generation);
    }

    /// See the bridge declaration.
    pub fn load_expansion(self: Pin<&mut Self>) {
        if *self.offline() {
            if let Some(wire) = self.rust().fixture_expansion.clone() {
                self.use_catalogue(&wire);
            }
            return;
        }
        service::request(
            self.qt_thread(),
            |proxy| async move {
                json::decode::<ExpansionCatalogue>(
                    "expansion catalogue",
                    &proxy.get_expansion_catalogue().await?,
                )
            },
            |mut backend, result| match result {
                Ok(wire) => backend.as_mut().use_catalogue(&wire),
                Err(error) => backend.as_mut().fail(&error),
            },
        );
    }

    /// See the bridge declaration.
    pub fn expansion_rows(&self) -> QString {
        self.rust().catalogue.as_ref().map_or_else(
            || q("[]"),
            |catalogue| q(&view::expansion_rows(self.snapshot(), catalogue)),
        )
    }

    /// See the bridge declaration.
    pub fn expansion_toggle(self: Pin<&mut Self>, id: &QString, enabled: bool) {
        let settings = self.snapshot().typed_config().advanced.expansion;
        let patch = expansion::toggle_patch(&settings, &id.to_string(), enabled);
        self.save(&patch);
    }

    /// See the bridge declaration.
    pub fn expansion_add_domain(self: Pin<&mut Self>, domain: &QString) -> QString {
        let settings = self.snapshot().typed_config().advanced.expansion;
        let Some(catalogue) = self.rust().catalogue.clone() else {
            return q("The catalogue is not loaded yet");
        };
        match expansion::normalise_domain(&domain.to_string(), &catalogue, &settings) {
            Ok(domain) => {
                self.save(&expansion::add_patch(&settings, &domain));
                QString::default()
            }
            Err(error) => q(&error.to_string()),
        }
    }

    /// See the bridge declaration.
    pub fn expansion_remove_domain(self: Pin<&mut Self>, domain: &QString) {
        let settings = self.snapshot().typed_config().advanced.expansion;
        self.save(&expansion::remove_patch(&settings, &domain.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_is_asked_for_by_scope() {
        // SCR-09, ADV-04
        assert_eq!(script_argument("global"), r#"{"scope":"global"}"#);
        assert_eq!(script_argument("rule:7"), r#"{"scope":"rule:7"}"#);
    }

    #[test]
    fn a_missing_feature_is_not_an_error() {
        assert!(is_absent(&Error::Unavailable(String::new())));
        assert!(is_absent(&Error::NotImplemented(String::new())));
        assert!(!is_absent(&Error::Failed(String::new())));
    }
}
