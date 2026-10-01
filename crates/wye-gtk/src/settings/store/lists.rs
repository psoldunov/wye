//! The shown browsers sheet (SHOWN-01 to SHOWN-08) and the app chooser
//! (DLG-APP) read and edit through these.

use gtk::subclass::prelude::*;
use wye_api::Error;
use wye_api::apps::AppList;

use super::SettingsStore;
use crate::service;
use crate::settings::menu;
use crate::settings::snapshot;
use crate::settings::{hotkeys, shown};

/// The shown browsers sheet (SHOWN-01 to SHOWN-08) and the app chooser
/// (DLG-APP): what `SettingsBackend.shown*`, `hotkeyChoices`, `checkHotkey`
/// and `AppChooserBackend` do on KDE, on the shared model.
impl SettingsStore {
    /// The shown list as the picker shows it: the stored one, or the
    /// installed browsers when nothing is stored.
    #[must_use]
    pub fn shown_entries(&self) -> Vec<shown::Entry> {
        self.with_snapshot(|s| {
            let foreign = menu::foreign_app_ids(&s.services.services);
            shown::effective(&s.targets, &shown::entries(&s.config), &foreign)
        })
    }

    /// The sheet's rows (SHOWN-02, SHOWN-03); under a hotkey scheme other
    /// than "Assigned per browser" each checked row carries the key the
    /// scheme gives it (SHOWN-04).
    #[must_use]
    pub fn shown_rows(&self) -> Vec<shown::Row> {
        let entries = self.shown_entries();
        self.with_snapshot(|s| {
            let foreign = menu::foreign_app_ids(&s.services.services);
            let rows = shown::rows(&s.targets, &entries, &foreign);
            let labels = hotkeys::scheme_labels(&s.typed_config(), &rows);
            if labels.is_empty() {
                rows
            } else {
                shown::with_scheme_hotkeys(rows, &labels)
            }
        })
    }

    /// Save the shown list `edit` makes of the current one (SHOWN-06: at
    /// once). The list is an array, so the whole list goes.
    pub fn edit_shown(&self, edit: impl FnOnce(&[shown::Entry]) -> Vec<shown::Entry>) {
        let entries = self.shown_entries();
        let next = edit(&entries);
        if next != entries {
            self.apply_patch(&shown::to_patch(&next));
        }
    }

    /// Whether the picker's hotkeys are assigned per browser (KEY-10): only
    /// then are the sheet's hotkey popups live (SHOWN-04).
    #[must_use]
    pub fn hotkeys_per_browser(&self) -> bool {
        self.with_snapshot(|s| {
            s.typed_config().picker.hotkeys == wye_core::config::HotkeyScheme::PerTarget
        })
    }

    /// The hotkey popup's letters and digits, without the keys picker
    /// actions use (SHOWN-04, KEY-12).
    #[must_use]
    pub fn hotkey_choices(&self) -> Vec<hotkeys::Choice> {
        self.with_snapshot(|s| hotkeys::choices(&s.typed_config()))
    }

    /// A key recorded with "Other Key…" as the stored hotkey.
    ///
    /// # Errors
    ///
    /// Not a single key, or one a picker action uses (KEY-12).
    pub fn check_hotkey(&self, recorded: &str) -> Result<String, hotkeys::HotkeyError> {
        self.with_snapshot(|s| hotkeys::check(&s.typed_config(), recorded))
    }

    /// The installed apps for the app chooser (DLG-APP): `GetApps(true)`, or
    /// the fixture's list while `offline`. `done` runs on the main thread.
    pub fn apps(&self, done: impl FnOnce(Result<AppList, Error>) + 'static) {
        if self.offline() {
            let apps = self.imp().fixture_apps.borrow().clone();
            done(Ok(apps.unwrap_or_default()));
            return;
        }
        service::request(
            |proxy| async move {
                let text = proxy.get_apps(true).await?;
                snapshot::decode_apps(&text)
            },
            done,
        );
    }
}
