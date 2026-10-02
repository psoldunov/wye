//! Which UI hosts serve the picker, the tray-menu popup and the windows, in
//! the order the service tries them (ADV-12). Pure: whether a host is
//! running or installed is checked by [`super::host`].

use wye_api::names::{
    GNOME_BUS_NAME, GNOME_OBJECT_PATH, GTK_BUS_NAME, GTK_OBJECT_PATH, UI_BUS_NAME, UI_OBJECT_PATH,
};
use wye_core::config::Frontend;

/// A process that serves `PickerHost1`, `Windows1` or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
    /// The GNOME Shell extension: `PickerHost1` while the Shell runs it.
    /// Never bus-activatable.
    Shell,
    /// `wye-gtk`: `Windows1`, and `PickerHost1` where it serves it.
    Gtk,
    /// `wye-ui` (Qt/Kirigami): both interfaces.
    Qt,
}

impl Host {
    /// Every host, for closing a picker wherever it shows.
    pub(crate) const ALL: [Self; 3] = [Self::Shell, Self::Gtk, Self::Qt];

    pub(crate) const fn bus_name(self) -> &'static str {
        match self {
            Self::Shell => GNOME_BUS_NAME,
            Self::Gtk => GTK_BUS_NAME,
            Self::Qt => UI_BUS_NAME,
        }
    }

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::Shell => GNOME_OBJECT_PATH,
            Self::Gtk => GTK_OBJECT_PATH,
            Self::Qt => UI_OBJECT_PATH,
        }
    }

    /// Whether D-Bus can start the host; the Shell extension only runs
    /// inside GNOME Shell.
    pub(crate) const fn activatable(self) -> bool {
        !matches!(self, Self::Shell)
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Shell => "GNOME Shell",
            Self::Gtk => "GTK",
            Self::Qt => "Qt",
        }
    }
}

/// ADV-12: picker and tray-popup hosts, preferred first. `auto` uses the
/// GNOME frontend in a GNOME Shell session (the Shell picker, else the GTK
/// host's, so one toolkit serves the picker and the windows while the
/// extension is off) and Qt alone elsewhere; `kde` and `gnome` put their
/// own frontend first on every desktop and the other one behind it, so a
/// link is never lost.
pub(crate) const fn picker_hosts(frontend: Frontend, gnome_session: bool) -> &'static [Host] {
    match (frontend, gnome_session) {
        (Frontend::Auto, true) | (Frontend::Gnome, _) => &[Host::Shell, Host::Gtk, Host::Qt],
        (Frontend::Auto, false) => &[Host::Qt],
        (Frontend::Kde, _) => &[Host::Qt, Host::Shell, Host::Gtk],
    }
}

/// PICK-25: whether the host that owned `bus_name` and just left is one the
/// warm-up keeps ready: a host D-Bus can start that may show the next
/// picker. Any other name leaving starts nothing.
pub(crate) fn rewarms(frontend: Frontend, gnome_session: bool, bus_name: &str) -> bool {
    picker_hosts(frontend, gnome_session)
        .iter()
        .any(|host| host.activatable() && host.bus_name() == bus_name)
}

/// ADV-12: window hosts, preferred first. GNOME's windows are GTK's.
pub(crate) const fn window_hosts(frontend: Frontend, gnome_session: bool) -> &'static [Host] {
    match (frontend, gnome_session) {
        (Frontend::Auto, true) | (Frontend::Gnome, _) => &[Host::Gtk, Host::Qt],
        (Frontend::Auto, false) => &[Host::Qt],
        (Frontend::Kde, _) => &[Host::Qt, Host::Gtk],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adv12_auto_keeps_the_desktops_own_frontend() {
        assert_eq!(
            picker_hosts(Frontend::Auto, true),
            [Host::Shell, Host::Gtk, Host::Qt]
        );
        assert_eq!(window_hosts(Frontend::Auto, true), [Host::Gtk, Host::Qt]);
        assert_eq!(picker_hosts(Frontend::Auto, false), [Host::Qt]);
        assert_eq!(window_hosts(Frontend::Auto, false), [Host::Qt]);
    }

    #[test]
    fn pick25_only_a_startable_picker_host_leaving_rewarms() {
        // KDE as before: wye-ui alone; a GTK host leaving starts nothing.
        assert!(rewarms(Frontend::Auto, false, UI_BUS_NAME));
        assert!(!rewarms(Frontend::Auto, false, GTK_BUS_NAME));
        assert!(!rewarms(Frontend::Auto, false, "org.example.Other"));
        // The GNOME frontend keeps GTK and Qt (its fallback) ready; the
        // Shell is never started.
        for gnome_session in [true, false] {
            assert!(rewarms(Frontend::Gnome, gnome_session, GTK_BUS_NAME));
            assert!(rewarms(Frontend::Gnome, gnome_session, UI_BUS_NAME));
            assert!(!rewarms(Frontend::Gnome, gnome_session, GNOME_BUS_NAME));
        }
        assert!(rewarms(Frontend::Auto, true, GTK_BUS_NAME));
    }

    #[test]
    fn adv12_a_chosen_frontend_applies_on_every_desktop_with_the_other_behind_it() {
        for gnome_session in [true, false] {
            assert_eq!(
                picker_hosts(Frontend::Kde, gnome_session),
                [Host::Qt, Host::Shell, Host::Gtk]
            );
            assert_eq!(
                window_hosts(Frontend::Kde, gnome_session),
                [Host::Qt, Host::Gtk]
            );
            assert_eq!(
                picker_hosts(Frontend::Gnome, gnome_session),
                [Host::Shell, Host::Gtk, Host::Qt]
            );
            assert_eq!(
                window_hosts(Frontend::Gnome, gnome_session),
                [Host::Gtk, Host::Qt]
            );
        }
    }

    #[test]
    fn only_the_shell_cannot_be_started() {
        let activatable: Vec<_> = Host::ALL.into_iter().filter(|h| h.activatable()).collect();
        assert_eq!(activatable, [Host::Gtk, Host::Qt]);
    }
}
