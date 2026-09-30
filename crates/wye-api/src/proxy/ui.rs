//! The UI host's interfaces (`wye-ui`, internal).

use zbus::proxy;

use crate::Error;

/// Shows the picker and the tray-menu popup.
#[proxy(
    interface = "dev.soldunov.wye.PickerHost1",
    default_service = "dev.soldunov.wye.Ui",
    default_path = "/dev/soldunov/wye/Ui"
)]
pub trait PickerHost1 {
    /// Show (or replace, PICK-27) the picker with a JSON
    /// [`crate::picker::PickerRequest`].
    fn show_picker(&self, request_id: &str, request: &str) -> Result<(), Error>;

    /// Close the picker for this request (superseded or screen locked).
    fn close_picker(&self, request_id: &str) -> Result<(), Error>;

    /// Toggle the tray-menu popup with a JSON [`crate::tray::TrayMenu`]
    /// (TRAY-08).
    fn show_menu(&self, menu: &str) -> Result<(), Error>;
}

/// Opens the UI host's windows.
#[proxy(
    interface = "dev.soldunov.wye.Windows1",
    default_service = "dev.soldunov.wye.Ui",
    default_path = "/dev/soldunov/wye/Ui"
)]
pub trait Windows1 {
    /// Open or raise a [`crate::actions::Window`].
    fn show_window(&self, window: &str, argument: &str) -> Result<(), Error>;

    /// Quit the UI host.
    fn quit(&self) -> Result<(), Error>;
}
