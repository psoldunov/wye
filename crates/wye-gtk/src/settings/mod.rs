//! The Settings window (03-settings-window.md).
//!
//! GTK-free logic, mirrored from crates/wye-ui/src/settings/ and tested
//! without a display:
//!
//! - [`snapshot`]: what the service said: configuration, status, targets,
//!   services; and how a patch changes it.
//! - [`patch`]: one control's change as an RFC 7386 merge patch (SET-06).
//! - [`save`]: saving a patch with `UpdateConfig`, and what a `Conflict`
//!   means (SET-06).
//! - [`sync`]: reloading and the buttons that act on the machine.
//! - [`menu`]: the target menu as rows (TGT-01 to TGT-07).
//! - [`help`]: the help popover texts (BLK-08, 19-help-texts.md).
//! - [`shown`], [`hotkeys`]: the shown browsers sheet's rows, edits and
//!   hotkey choices (SHOWN-01 to SHOWN-08).
//! - [`chooser`]: the app chooser's sections and search (DLG-APP).
//! - [`icon`]: target icons as names or paths.
//! - [`fixture`]: the self-test's stand-in for the service.
//! - [`request`]: what a `ShowWindow` asks of this window.
//!
//! On GTK:
//!
//! - [`store`]: [`store::SettingsStore`], the window's data and the one way
//!   to change it; every page shares it.
//! - [`window`]: the window frame, switcher, banners and shortcuts.
//! - [`pages`]: the seven pages.
//! - [`sheets`]: sheets more than one page opens (the app chooser).

// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The app chooser's sections and search (DLG-APP-01 to DLG-APP-04).
#[path = "../../../wye-ui/src/settings/chooser.rs"]
pub mod chooser;
#[allow(
    dead_code,
    reason = "mirrors crates/wye-ui/src/settings/fixture.rs whole"
)]
pub mod fixture;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/settings/help.rs"]
pub mod help;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The shown browsers' hotkey popup (SHOWN-04, KEY-10, KEY-12).
#[path = "../../../wye-ui/src/settings/hotkeys.rs"]
pub mod hotkeys;
pub mod icon;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink: `#[path]` would make rustc look for the nested
// `mod tests;` beside wye-ui's file, where `settings/tests.rs` does not exist.
pub mod menu;
pub mod pages;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[allow(
    dead_code,
    reason = "the file is shared whole; the GTK pages use only part of it"
)]
#[path = "../../../wye-ui/src/settings/patch.rs"]
pub mod patch;
pub mod request;
pub mod sheets;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The shown browsers sheet's rows and edits (SHOWN-01 to SHOWN-08). The file
// is a symlink, so its nested `mod tests;` is found in `shown/tests.rs`.
pub mod shown;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink: `#[path]` would make rustc look for the nested
// `mod tests;` beside wye-ui's file, where `settings/tests.rs` does not exist.
#[allow(
    dead_code,
    reason = "the file is shared whole; the GTK pages use only part of it"
)]
pub mod save;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[allow(
    dead_code,
    reason = "the file is shared whole; the GTK pages use only part of it"
)]
#[path = "../../../wye-ui/src/settings/snapshot.rs"]
pub mod snapshot;
pub mod store;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink: `#[path]` would make rustc look for the nested
// `mod tests;` beside wye-ui's file, where `settings/tests.rs` does not exist.
pub mod sync;
pub mod window;

use std::cell::OnceCell;
use std::rc::Rc;

use crate::app::Presenter;
use window::SettingsWindow;

/// The Settings surface: one window, created on first use (SET-04).
#[derive(Debug)]
pub struct Settings {
    app: adw::Application,
    window: OnceCell<Rc<SettingsWindow>>,
}

impl Settings {
    /// The surface for `app`; nothing is built until it is shown.
    #[must_use]
    pub fn new(app: &adw::Application) -> Self {
        Self {
            app: app.clone(),
            window: OnceCell::new(),
        }
    }
}

impl Presenter for Settings {
    fn present(&self, key: &str, argument: &str) {
        self.window
            .get_or_init(|| SettingsWindow::new(&self.app))
            .handle(key, argument);
    }
}
