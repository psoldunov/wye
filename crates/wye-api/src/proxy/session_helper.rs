//! The GNOME Shell extension's session helper (internal).

use zbus::proxy;

use crate::Error;

/// What the Shell knows and the service cannot ask Mutter for: the pointer,
/// held modifiers, the focused app and the clipboard. Only the owner of
/// `dev.soldunov.wye` may call it.
#[proxy(
    interface = "dev.soldunov.wye.SessionHelper1",
    default_service = "dev.soldunov.wye.Gnome",
    default_path = "/dev/soldunov/wye/Gnome"
)]
pub trait SessionHelper1 {
    /// The pointer: `x` and `y` in the logical coordinates of `output`,
    /// a connector name such as `DP-1` (PICK-02).
    fn query_pointer(&self) -> Result<(i32, i32, String), Error>;

    /// The modifiers held now, as [`crate::context::Modifier`] names.
    fn query_modifiers(&self) -> Result<Vec<String>, Error>;

    /// The desktop ID of the focused window's app; empty when unknown
    /// (source-app step 4).
    fn focused_app(&self) -> Result<String, Error>;

    /// The clipboard's text; empty when it holds none, or a password
    /// manager's secret or an image (EXT-12).
    fn read_clipboard(&self) -> Result<String, Error>;

    /// Replace the clipboard's text (EXT-12, PICK-28 Copy Link).
    fn write_clipboard(&self, text: &str) -> Result<(), Error>;

    /// Start or stop sending [`Self::clipboard_changed`] to the caller.
    fn watch_clipboard(&self, watch: bool) -> Result<(), Error>;

    /// New clipboard text, sent to the watching service alone.
    #[zbus(signal)]
    fn clipboard_changed(&self, text: &str) -> zbus::Result<()>;
}
