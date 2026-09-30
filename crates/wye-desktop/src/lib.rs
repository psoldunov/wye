//! Wye's Linux desktop integration: desktop entries, browser discovery and
//! profiles, launching, the default-browser setting and source-app
//! detection.
//!
//! Nothing here reads the environment or the real filesystem implicitly:
//! every function takes an [`XdgDirs`] value or a `/proc` root, and only
//! [`XdgDirs::from_env`] reads the process environment. The one exception is
//! the loop guard, which compares commands with the running executable
//! ([`loop_guard::runs_current_exe`]). Requirement IDs such
//! as `DISC-05` refer to the specification in `docs/spec/`.

pub mod atomic;
pub mod autostart;
pub mod default_browser;
pub mod discovery;
pub mod entry;
pub mod exec;
pub mod family;
pub mod kdeglobals;
pub mod keyfile;
pub mod launch;
pub mod loop_guard;
pub mod native_messaging;
pub mod profiles;
pub mod source_app;
pub mod stand_in;
pub mod state;
pub mod xdg;

#[cfg(test)]
mod test_support;

pub use default_browser::{
    DefaultBrowserError, current_default, is_default, listed_default, set_default,
};
pub use discovery::{InstalledApp, Inventory, PrivateMode, find_entry};
pub use entry::{DesktopAction, DesktopEntry, EntryError};
pub use exec::{ExecContext, ExecError, ExecTemplate};
pub use family::{BrowserFamily, Packaging};
pub use launch::{LaunchCommand, LaunchError, LaunchRequest, WYE_DESKTOP_ID, build_command, spawn};
pub use loop_guard::forwards_links;
pub use profiles::{Profile, ProfileError};
pub use source_app::{ExecMatcher, ProcessProgram};
pub use state::{State, StateError};
pub use xdg::{Locale, NoHomeError, XdgDirs};
