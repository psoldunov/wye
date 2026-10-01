//! Calls the Picker, Extras and Advanced pages make that only act on the
//! service (Preview Picker, Clear History, Show…), and what those pages read
//! from it: global shortcuts (ADV-05 to ADV-07), the expansion catalogue
//! (DLG-EXP), section defaults (KEY-04).

use gtk::glib;
use gtk::subclass::prelude::*;
use serde_json::Value;
use wye_api::Error;

use super::SettingsStore;
use crate::error_text::ErrorText;
use crate::service;
use crate::settings::patch;

/// What the pages call on the service.
impl SettingsStore {
    /// A call the user asked for that only acts on the service (Preview
    /// Picker, Clear History, Show…). Nothing happens while `offline`; a
    /// failure shows in the window's banner.
    pub fn call<W, F>(&self, work: W)
    where
        W: FnOnce(wye_api::proxy::Wye1Proxy<'static>) -> F + Send + 'static,
        F: std::future::Future<Output = Result<(), Error>> + Send + 'static,
    {
        if self.offline() {
            return;
        }
        service::request(
            work,
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

    /// Ask the service to show the frontend's window `window` with
    /// `argument` (`ShowWindow`: History, the script editor; ADV-04, ADV-09).
    pub fn show_window(&self, window: &'static str, argument: String) {
        self.call(move |proxy| async move { proxy.show_window(window, &argument).await });
    }

    /// Whether the script of `scope` exists (SCR-09): `done(false)` only when
    /// the service says it does not; nothing while `offline` or when the
    /// service cannot say.
    pub fn script_exists(&self, scope: &str, done: impl FnOnce(bool) + 'static) {
        if self.offline() {
            return;
        }
        let scope = scope.to_owned();
        service::request(
            move |proxy| async move { proxy.script_exists(&scope).await },
            move |result| match result {
                Ok(exists) => done(exists),
                Err(error) => tracing::debug!(%error, "cannot tell whether the script exists"),
            },
        );
    }

    /// The global shortcuts (ADV-05 to ADV-07, KEY-40): `GetShortcuts`, or
    /// the fixture's while `offline`; `None` when the service cannot say.
    pub fn shortcuts(&self, done: impl FnOnce(Option<wye_api::shortcuts::Shortcuts>) + 'static) {
        if self.offline() {
            done(self.imp().fixture_shortcuts.borrow().clone());
            return;
        }
        service::request(
            |proxy| async move {
                let text = proxy.get_shortcuts().await?;
                wye_api::json::decode::<wye_api::shortcuts::Shortcuts>("shortcuts", &text)
            },
            move |result| match result {
                Ok(wire) => done(Some(wire)),
                Err(error) => {
                    tracing::debug!(%error, "no global shortcuts to list");
                    done(None);
                }
            },
        );
    }

    /// Bind the global shortcut `action` to `binding` (an empty one clears
    /// it), then run `done` to read the shortcuts again. While `offline` the
    /// configuration's `shortcuts` table takes it. A session without a
    /// mechanism refuses it; the row then shows the command to bind by hand
    /// (KEY-41), so that refusal is not an error.
    pub fn set_shortcut(&self, action: &str, binding: &str, done: impl FnOnce() + 'static) {
        if self.offline() {
            let value = if binding.is_empty() {
                Value::Null
            } else {
                Value::from(binding)
            };
            self.set_value(&format!("shortcuts.{action}"), value);
            done();
            return;
        }
        let (action, binding) = (action.to_owned(), binding.to_owned());
        service::request(
            move |proxy| async move { proxy.set_shortcut(&action, &binding).await },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| {
                    if let Err(error) = result
                        && !matches!(error, Error::Unavailable(_) | Error::NotImplemented(_))
                    {
                        store.fail(&error);
                    }
                    done();
                }
            ),
        );
    }

    /// The URL expansion catalogue (DLG-EXP): `GetExpansionCatalogue`, or
    /// the fixture's while `offline`. A failure shows in the banner.
    pub fn expansion_catalogue(
        &self,
        done: impl FnOnce(wye_api::expansion::ExpansionCatalogue) + 'static,
    ) {
        if self.offline() {
            if let Some(wire) = self.imp().fixture_expansion.borrow().clone() {
                done(wire);
            }
            return;
        }
        service::request(
            |proxy| async move {
                let text = proxy.get_expansion_catalogue().await?;
                wye_api::json::decode::<wye_api::expansion::ExpansionCatalogue>(
                    "expansion catalogue",
                    &text,
                )
            },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| match result {
                    Ok(wire) => done(wire),
                    Err(error) => store.fail(&error),
                }
            ),
        );
    }

    /// Return the section at the dotted `path` (`picker.keys`) to its
    /// defaults (KEY-04): the service's (`GetDefaults`), or the built-in
    /// ones while `offline`.
    pub fn reset_section(&self, path: &str) {
        if self.offline() {
            let built = patch::local_defaults(path).and_then(|defaults| patch::set(path, defaults));
            self.save_built(built);
            return;
        }
        let path = path.to_owned();
        let asked = path.clone();
        service::request(
            move |proxy| async move {
                let text = proxy.get_defaults(&asked).await?;
                wye_api::json::decode::<Value>("defaults", &text)
            },
            glib::clone!(
                #[weak(rename_to = store)]
                self,
                move |result| match result {
                    Ok(defaults) => store.save_built(patch::set(&path, defaults)),
                    Err(error) => store.fail(&error),
                }
            ),
        );
    }

    /// Show `message` in the window's banner: a change a page refused to
    /// build (an invalid binding, say).
    pub fn report(&self, message: &str) {
        self.set_error(ErrorText::plain(message.to_owned()));
    }
}
