//! Wye's internal state, kept apart from the configuration in
//! `$XDG_STATE_HOME/wye/state.toml` so a read-only `config.toml` (for
//! example one written by home-manager) never blocks it (12-data-model.md,
//! "Internal").
//!
//! The CLI (`wye default`, `wye extension`) and the service share this
//! file. Every writer loads the whole state, changes its own fields and
//! saves the result, and holds an exclusive lock (`state.toml.lock` next to
//! the file, see [`StateLock`]) for that whole cycle, so a save never writes
//! back a copy that is stale: one writer never drops what another keeps.
//! [`State::update`] is that cycle; [`State::save`] takes the lock too.
//! Readers take no lock: the atomic rename always shows them a whole file.
//!
//! A writer never replaces a file it cannot read. Its fields, notably
//! `previous-default-browser` (DEF-05), may be the only record of what the
//! user had before Wye; the writer reports the error and leaves the file be.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
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
    #[error("cannot lock {path}: {source}")]
    Lock {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// The exclusive lock every writer of the state file holds while it loads,
/// changes and saves it. Released when dropped.
///
/// The lock is an `flock` on `state.toml.lock`, next to the state file. It
/// is a separate file because [`atomic::write`] replaces `state.toml`'s
/// inode on every save, so a lock on that file would not outlast a write.
/// The lock file is left in place on purpose: removing it would let two
/// writers lock different inodes. `flock` locks belong to the open file
/// description, so two holders in one process exclude each other too.
///
/// A holder must not call [`State::save`] or [`State::update`] on the same
/// path, nor acquire a second `StateLock` for it: each waits for the lock,
/// so it would wait on itself forever. Save through the guard instead.
#[derive(Debug)]
#[must_use = "the lock is released as soon as the guard is dropped"]
pub struct StateLock {
    path: PathBuf,
    // Held for its `flock`, released when the file closes.
    _file: File,
}

impl StateLock {
    /// Waits for the exclusive lock of the state file at `path` (the state
    /// file, not the lock file), creating the directory and the lock file
    /// when needed.
    ///
    /// # Errors
    ///
    /// [`StateError::Lock`] when the directory or lock file cannot be
    /// created or the lock cannot be taken.
    pub fn acquire(path: &Path) -> Result<Self, StateError> {
        let mut lock_path = path.as_os_str().to_owned();
        lock_path.push(".lock");
        let lock_path = PathBuf::from(lock_path);
        let locked = |source| StateError::Lock {
            path: lock_path.clone(),
            source,
        };
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(locked)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&lock_path)
            .map_err(locked)?;
        loop {
            match flock(&file, FlockOperation::LockExclusive) {
                Ok(()) => break,
                Err(Errno::INTR) => {}
                Err(errno) => return Err(locked(errno.into())),
            }
        }
        Ok(Self {
            path: path.to_owned(),
            _file: file,
        })
    }

    /// Reads the state file under the lock; see [`State::load`].
    ///
    /// # Errors
    ///
    /// As [`State::load`].
    pub fn load(&self) -> Result<State, StateError> {
        State::load(&self.path)
    }

    /// Writes the state file under the lock; see [`State::save`].
    ///
    /// # Errors
    ///
    /// [`StateError::Serialize`] when the state cannot be serialised and
    /// [`StateError::Write`] when the directory or file cannot be written.
    pub fn save(&self, state: &State) -> Result<(), StateError> {
        write(&self.path, state)
    }
}

/// Writes `state` atomically, creating the directory when needed. The caller
/// holds the lock.
fn write(path: &Path, state: &State) -> Result<(), StateError> {
    let text = toml::to_string(state)?;
    atomic::write(path, text.as_bytes(), Some(path)).map_err(|source| StateError::Write {
        path: path.to_owned(),
        source,
    })
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
    /// This replaces every field, waiting for the lock so it never lands
    /// inside another writer's update. To change some fields, keeping the
    /// rest as they are on disk, use [`State::update`].
    ///
    /// # Errors
    ///
    /// [`StateError::Lock`] when the lock cannot be taken,
    /// [`StateError::Serialize`] when the state cannot be serialised and
    /// [`StateError::Write`] when the directory or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        StateLock::acquire(path)?.save(self)
    }

    /// Changes the state file: under the lock, loads it, applies `change` and
    /// saves the result when it differs. Returns the resulting state.
    ///
    /// An unreadable file is an error and is left as it is. The lock is
    /// taken even when nothing changes, so a writable state directory is
    /// needed, as for every writer.
    ///
    /// # Errors
    ///
    /// [`StateError::Lock`], the errors of [`State::load`],
    /// [`StateError::Serialize`] and [`StateError::Write`].
    pub fn update(path: &Path, change: impl FnOnce(Self) -> Self) -> Result<Self, StateError> {
        let lock = StateLock::acquire(path)?;
        let before = lock.load()?;
        let after = change(before.clone());
        if after != before {
            lock.save(&after)?;
        }
        Ok(after)
    }
}

#[cfg(test)]
mod tests;
