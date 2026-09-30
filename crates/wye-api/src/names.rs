//! Bus names, object paths and interface names (`docs/dbus-api.md`).
//!
//! The proxies in [`crate::proxy`] repeat these as literals, because zbus
//! attributes take no constants; a test checks that both agree.

/// The service's well-known name (DEF-04). Also the application ID.
pub const BUS_NAME: &str = "dev.soldunov.wye";

/// The service's only object.
pub const OBJECT_PATH: &str = "/dev/soldunov/wye";

/// The public interface on [`OBJECT_PATH`].
pub const INTERFACE: &str = "dev.soldunov.wye1";

/// Desktop activation on [`OBJECT_PATH`] (`DBusActivatable=true`).
pub const APPLICATION_INTERFACE: &str = "org.freedesktop.Application";

/// Internal interface the one-shot `KWin` script calls back on.
pub const KWIN_INTERFACE: &str = "dev.soldunov.wye.KWin1";

/// The UI host's well-known name (`wye-ui`).
pub const UI_BUS_NAME: &str = "dev.soldunov.wye.Ui";

/// The UI host's object.
pub const UI_OBJECT_PATH: &str = "/dev/soldunov/wye/Ui";

/// Shows and closes the picker and the tray-menu popup. Served by `wye-ui`,
/// later also by the GNOME Shell extension under its own name.
pub const PICKER_HOST_INTERFACE: &str = "dev.soldunov.wye.PickerHost1";

/// Opens the UI host's windows.
pub const WINDOWS_INTERFACE: &str = "dev.soldunov.wye.Windows1";

/// Reserved for the GNOME Shell extension (pointer, modifiers, focus,
/// clipboard). Not implemented yet.
pub const SESSION_HELPER_INTERFACE: &str = "dev.soldunov.wye.SessionHelper1";

/// Wye's desktop entry.
pub const DESKTOP_ID: &str = "dev.soldunov.wye.desktop";

/// Exit status of `wye service` when another instance owns [`BUS_NAME`].
///
/// `wye.service` lists it in `RestartPreventExitStatus=`: restarting would
/// only lose the same race again.
pub const ALREADY_RUNNING_EXIT: u8 = 75;
