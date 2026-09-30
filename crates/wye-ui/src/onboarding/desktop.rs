//! What the desktop integration step says (ONB-04): which desktop this is,
//! and what its tray needs.

use serde::Serialize;

/// The variable that names the desktop (XDG Desktop Entry spec).
pub const CURRENT_DESKTOP: &str = "XDG_CURRENT_DESKTOP";

/// The desktop family the step tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Desktop {
    Kde,
    Gnome,
    Other,
}

impl Desktop {
    /// The desktop `XDG_CURRENT_DESKTOP` names: a colon-separated list, of
    /// which the first known entry counts.
    #[must_use]
    pub fn of(current_desktop: &str) -> Self {
        current_desktop
            .split(':')
            .find_map(|name| match name.to_ascii_lowercase().as_str() {
                "kde" => Some(Self::Kde),
                "gnome" => Some(Self::Gnome),
                _ => None,
            })
            .unwrap_or(Self::Other)
    }

    /// The desktop of this session.
    #[must_use]
    pub fn detect() -> Self {
        std::env::var(CURRENT_DESKTOP).map_or(Self::Other, |value| Self::of(&value))
    }

    /// The note the step shows below **Launch at login**, if any (ONB-04).
    /// Plasma has a tray, so it needs none.
    #[must_use]
    pub const fn note(self) -> Option<&'static str> {
        match self {
            Self::Kde => None,
            Self::Gnome => Some(
                "GNOME Shell integration adds the tray icon, the picker at the pointer, held keys and clipboard features. Wye's Shell extension is not available yet.",
            ),
            Self::Other => Some(
                "Wye's tray icon needs a system tray. Without one, open Wye's windows from its application menu.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_is_read_from_the_colon_separated_list() {
        assert_eq!(Desktop::of("KDE"), Desktop::Kde);
        assert_eq!(Desktop::of("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(Desktop::of("sway"), Desktop::Other);
        assert_eq!(Desktop::of(""), Desktop::Other);
    }

    #[test]
    fn only_desktops_without_a_ready_tray_get_a_note() {
        // ONB-04
        assert_eq!(Desktop::Kde.note(), None);
        assert!(Desktop::Gnome.note().is_some_and(|n| n.contains("Shell")));
        assert!(
            Desktop::Other
                .note()
                .is_some_and(|n| n.contains("system tray"))
        );
    }
}
