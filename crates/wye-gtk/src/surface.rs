//! The surfaces `wye-gtk` shows: one top-level window or dialog each, created
//! on first use and kept for the life of the process (SET-04). Sheets (the
//! rule editor, the tester, the app chooser) live inside the Settings window
//! and are not surfaces of their own.
//!
//! The picker and the tray-menu popup are surfaces too, for sessions where
//! the GNOME Shell extension does not draw them (`crate::host`, ADV-12).
//! Neither is a window name: only `PickerHost1` routes to them.

use wye_api::actions::Window;

/// One window or dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
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
    /// The widget kit's gallery: the self-test's only, no window name routes
    /// here (`crate::selftest::kit`).
    Kit,
    /// The picker (02-picker.md), from `PickerHost1.ShowPicker`.
    Picker,
    /// The tray-menu popup (TRAY-08), from `PickerHost1.ShowMenu`.
    TrayMenu,
}

impl Surface {
    /// Every surface, in the order the self-test shows them.
    pub const ALL: [Self; 8] = [
        Self::Settings,
        Self::ScriptEditor,
        Self::History,
        Self::About,
        Self::Onboarding,
        Self::Kit,
        Self::Picker,
        Self::TrayMenu,
    ];

    /// The name the self-test uses; also the fixture's file stem.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::ScriptEditor => "script-editor",
            Self::History => "history",
            Self::About => "about",
            Self::Onboarding => "onboarding",
            Self::Kit => "kit",
            Self::Picker => "picker",
            Self::TrayMenu => "tray-menu",
        }
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_and_round_trip() {
        let names: HashSet<_> = Surface::ALL.map(Surface::name).into_iter().collect();
        assert_eq!(names.len(), Surface::ALL.len());
        for surface in Surface::ALL {
            assert_eq!(Surface::from_name(surface.name()), Some(surface));
        }
        assert_eq!(Surface::from_name("tray"), None);
    }

    #[test]
    fn rule_editor_and_tester_open_in_settings() {
        // SET-04: one Settings window; its sheets are not windows of their own.
        assert_eq!(Surface::for_window(Window::RuleEditor), Surface::Settings);
        assert_eq!(Surface::for_window(Window::TestRules), Surface::Settings);
        assert_eq!(Surface::for_window(Window::FirstRun), Surface::Onboarding);
    }
}
