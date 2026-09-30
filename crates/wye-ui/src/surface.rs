//! The surfaces `wye-ui` shows, and the QML file each one starts from.
//!
//! A surface is one top-level window or popup with its own directory under
//! `qml/`. Sheets (app chooser, rule editor, tester, URL expansion) live
//! inside the Settings window and are not surfaces of their own.

use wye_api::actions::Window;

/// Where the QML module's files are in the Qt resource system: cxx-qt-build
/// registers the module `dev.soldunov.wye.ui` under `/qt/qml/<uri as dirs>/`
/// and keeps each file's path relative to the crate.
const MODULE_ROOT_URL: &str = "qrc:/qt/qml/dev/soldunov/wye/ui/";

/// The file `QQmlApplicationEngine` loads.
pub const MAIN_QML: &str = "qml/Main.qml";

/// One window or popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    /// The picker (02-picker.md).
    Picker,
    /// The tray-menu popup (TRAY-08).
    TrayMenu,
    /// Settings, with its sheets (03-settings-window.md).
    Settings,
    /// The script editor (16-script-editor.md).
    ScriptEditor,
    /// History (DLG-HIS).
    History,
    /// About (DLG-ABT).
    About,
    /// First run (18-onboarding.md).
    Onboarding,
}

impl Surface {
    /// Every surface, in the order the self-test loads them.
    pub const ALL: [Self; 7] = [
        Self::Picker,
        Self::TrayMenu,
        Self::Settings,
        Self::ScriptEditor,
        Self::History,
        Self::About,
        Self::Onboarding,
    ];

    /// The name QML and the self-test use; also the fixture's file stem.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Picker => "picker",
            Self::TrayMenu => "tray-menu",
            Self::Settings => "settings",
            Self::ScriptEditor => "script-editor",
            Self::History => "history",
            Self::About => "about",
            Self::Onboarding => "onboarding",
        }
    }

    /// The surface's root QML file, relative to the crate.
    pub const fn qml_file(self) -> &'static str {
        match self {
            Self::Picker => "qml/picker/PickerWindow.qml",
            Self::TrayMenu => "qml/traymenu/TrayMenuPopup.qml",
            Self::Settings => "qml/settings/SettingsWindow.qml",
            Self::ScriptEditor => "qml/script/ScriptEditorWindow.qml",
            Self::History => "qml/history/HistoryWindow.qml",
            Self::About => "qml/about/AboutWindow.qml",
            Self::Onboarding => "qml/onboarding/OnboardingWindow.qml",
        }
    }

    /// The root file's `qrc:` URL.
    pub fn url(self) -> String {
        resource_url(self.qml_file())
    }

    /// The surface called `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|surface| surface.name() == name)
    }

    /// The surface that shows `window` (`Windows1.ShowWindow`). The rule
    /// editor and the rule tester are sheets of the Settings window.
    pub const fn for_window(window: Window) -> Self {
        match window {
            Window::Settings | Window::RuleEditor | Window::TestRules => Self::Settings,
            Window::FirstRun => Self::Onboarding,
            Window::History => Self::History,
            Window::About => Self::About,
            Window::ScriptEditor => Self::ScriptEditor,
        }
    }
}

/// The `qrc:` URL of a file of the QML module, given relative to the crate.
pub fn resource_url(file: &str) -> String {
    format!("{MODULE_ROOT_URL}{file}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::*;

    #[test]
    fn every_root_file_exists() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        for file in Surface::ALL
            .map(Surface::qml_file)
            .into_iter()
            .chain([MAIN_QML])
        {
            assert!(crate_dir.join(file).is_file(), "{file} is missing");
        }
    }

    #[test]
    fn names_are_unique_and_round_trip() {
        let names: HashSet<_> = Surface::ALL.map(Surface::name).into_iter().collect();
        assert_eq!(names.len(), Surface::ALL.len());
        for surface in Surface::ALL {
            assert_eq!(Surface::from_name(surface.name()), Some(surface));
        }
        assert_eq!(Surface::from_name("Settings"), None);
    }

    #[test]
    fn root_file_names_are_unique_qml_types() {
        // Every file of the module becomes a type named after its stem, so
        // two files with the same name in different directories clash.
        let stems: HashSet<_> = Surface::ALL
            .map(|surface| {
                Path::new(surface.qml_file())
                    .file_stem()
                    .map(ToOwned::to_owned)
            })
            .into_iter()
            .collect();
        assert_eq!(stems.len(), Surface::ALL.len());
    }

    #[test]
    fn urls_point_into_the_module() {
        assert_eq!(
            Surface::Picker.url(),
            "qrc:/qt/qml/dev/soldunov/wye/ui/qml/picker/PickerWindow.qml"
        );
    }

    #[test]
    fn rule_editor_and_tester_open_in_settings() {
        // SET-04: one Settings window; its sheets are not windows of their own.
        assert_eq!(Surface::for_window(Window::RuleEditor), Surface::Settings);
        assert_eq!(Surface::for_window(Window::TestRules), Surface::Settings);
        assert_eq!(Surface::for_window(Window::FirstRun), Surface::Onboarding);
    }
}
