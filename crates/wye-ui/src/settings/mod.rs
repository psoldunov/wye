//! The Settings window's logic on the UI side (03-settings-window.md), kept
//! out of the bridge so it is tested without Qt. `bridge/settings.rs` and
//! `bridge/app_chooser.rs` convert types and call it.
//!
//! - [`snapshot`]: what the service said: configuration, status, targets,
//!   services; and how a patch changes it.
//! - [`patch`]: one control's change as an RFC 7386 merge patch (SET-06).
//! - [`menu`]: the target menu as rows (TGT-01 to TGT-07).
//! - [`shown`]: the shown browsers sheet as rows and its edits (SHOWN-01 to
//!   SHOWN-08).
//! - [`hotkeys`]: the hotkey popup's choices (SHOWN-04, KEY-10, KEY-12).
//! - [`chooser`]: the app chooser's sections and search (DLG-APP-01 to 04).
//! - [`help`]: the help popover texts (BLK-08, 19-help-texts.md).
//! - [`keys`]: the picker keys sheet's clash rules and patches (KEY-20 to KEY-22).
//! - [`shortcuts`]: the global shortcut rows (ADV-05 to ADV-07, KEY-40, KEY-41).
//! - [`expansion`]: the URL expansion sheet's rows and edits (DLG-EXP).
//! - [`controls`]: the bridge's invokables that save one control's change and
//!   the queries the controls read.
//! - [`pages`]: the bridge's invokables for the Picker, Extras and Advanced pages.
//! - [`record`]: a key press as a stored binding (KEY-02, KEY-03).
//! - [`icon`]: icon names and paths as QML draws them.
//! - [`save`]: saving a patch with `UpdateConfig`, and what a `Conflict` means
//!   (SET-06); every window that writes the configuration uses it.
//! - [`sync`]: reloading and the buttons that act on the machine, over D-Bus
//!   (tested with a fake).
//! - [`view`]: the JSON QML reads, built from a snapshot.
//! - [`fixture`]: the self-test's stand-in for the service.

pub mod chooser;
pub mod controls;
pub mod expansion;
pub mod fixture;
pub mod help;
pub mod hotkeys;
pub mod icon;
pub mod keys;
pub mod menu;
pub mod pages;
pub mod patch;
pub mod record;
pub mod save;
pub mod shortcuts;
pub mod shown;
pub mod snapshot;
pub mod sync;
pub mod view;
