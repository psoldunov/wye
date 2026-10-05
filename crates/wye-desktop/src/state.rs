//! Wye's internal state, kept apart from the configuration in
//! `$XDG_STATE_HOME/wye/state.toml` so a read-only `config.toml` (for
//! example one written by home-manager) never blocks it (12-data-model.md,
//! "Internal").
//!
//! The CLI (`wye default`) and the service share this file. Every writer
//! loads the whole state, changes its own fields and saves the result, so
//! one never drops what the other keeps.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use wye_core::DesktopId;

use crate::atomic;

/// The state file's contents. Absent keys take their defaults, and values
/// that equal their defaults are not written, so the file stays short.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct State {
    /// The default browser Wye replaced, restored by "Stop Being Default"
    /// and `wye default unset` (DEF-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_default_browser: Option<DesktopId>,
    /// Plasma's `BrowserApplication` before Wye set it, as stored (DEF-02,
    /// DEF-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_kdeglobals_browser: Option<String>,
    /// The first-run window was finished (ONB-06).
    #[serde(skip_serializing_if = "is_false")]
    pub onboarding_done: bool,
    /// IDs of callouts the user closed (BLK-09).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dismissed_callouts: Vec<String>,
    /// The Settings page shown last (SET-08).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_settings_page: Option<String>,
    /// The rules help arrow was seen (RUL-19).
    #[serde(skip_serializing_if = "is_false")]
    pub rules_help_seen: bool,
    /// The app the user chose to keep as the default browser with "Keep
    /// <App>" (ONB-11); no takeover notification while it stays the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_default: Option<DesktopId>,
    /// Hashes of scripts whose failure was already notified (SCR-22).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub script_errors_notified: Vec<String>,
    /// The user removed the browser extension's host manifests (`wye
    /// extension remove`), so the service no longer writes them at start
    /// until `wye extension install` (BEXT-04).
    #[serde(skip_serializing_if = "is_false")]
    pub extension_host_removed: bool,
    /// Web-link handlers discovery already offered for the picker
    /// (SHOWN-09); `None` until the first scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seen_browsers: Option<Vec<DesktopId>>,
}

/// The state file `$XDG_STATE_HOME/wye/state.toml` (`state_home` when it
/// is absolute), else `~/.local/state/wye/state.toml` under `home`.
#[must_use]
pub fn path(home: &Path, state_home: Option<&Path>) -> PathBuf {
    state_home
        .filter(|dir| dir.is_absolute())
        .map_or_else(|| home.join(".local").join("state"), Path::to_path_buf)
        .join("wye")
        .join("state.toml")
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde hands `skip_serializing_if` a reference"
)]
const fn is_false(value: &bool) -> bool {
    !*value
}

/// The state file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path} is not a valid state file: {source}")]
    Invalid {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialise the state: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl State {
    /// Reads the state file; a missing file is the empty state.
    ///
    /// # Errors
    ///
    /// [`StateError::Read`] when the file cannot be read and
    /// [`StateError::Invalid`] when it is not a state file.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| StateError::Invalid {
                path: path.to_owned(),
                source,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(StateError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes the state atomically, creating its directory when needed. A
    /// symlink at `path` is left alone and reported as an error.
    ///
    /// # Errors
    ///
    /// [`StateError::Write`] when the directory or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        let text = toml::to_string(self)?;
        atomic::write(path, text.as_bytes(), Some(path)).map_err(|source| StateError::Write {
            path: path.to_owned(),
            source,
        })
    }
}

#[cfg(test)]
mod tests;
