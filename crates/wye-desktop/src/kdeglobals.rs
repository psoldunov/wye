//! Plasma's own default-browser setting (DEF-02): `BrowserApplication` in the
//! `[General]` group of `$XDG_CONFIG_HOME/kdeglobals`.
//!
//! KDE applications (KIO's `OpenUrlJob`, and `xdg-settings` on Plasma) read
//! this key before they fall back to the `text/html` association, so on KDE
//! it is set next to `mimeapps.list`. The value is the desktop file name,
//! `firefox.desktop`; older setups hold a command prefixed with `!`. The
//! previous value is returned to the caller, who keeps it (DEF-05) and hands
//! it back to [`restore_browser`] when Wye stops being the default.
//!
//! Only `kdeglobals` itself is touched, and only when the session is KDE
//! (`XDG_CURRENT_DESKTOP` contains `KDE`). The change goes through
//! `kwriteconfig6` when it is on `PATH` (always on Plasma), which takes
//! `KConfig`'s lock, so a Plasma process saving `kdeglobals` at the same
//! moment loses nothing. Without it, every other line is kept byte for byte
//! and the file is replaced atomically. A symlink or read-only file is
//! managed elsewhere (home-manager, say) and gives
//! [`DefaultBrowserError::Managed`].

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use wye_core::DesktopId;

use crate::default_browser::{
    DefaultBrowserError, ensure_writable, read_existing, remove_key, set_keys,
};
use crate::xdg::XdgDirs;
use crate::{atomic, keyfile};

const FILE: &str = "kdeglobals";
/// Plasma's command-line `KConfig` writer.
const KWRITECONFIG: &str = "kwriteconfig6";
const GROUP: &str = "General";
/// The key KDE reads the default browser from.
pub const BROWSER_KEY: &str = "BrowserApplication";
/// The same key with `KConfig`'s "expand variables" flag, which some files
/// carry.
const BROWSER_KEY_EXPANDED: &str = "BrowserApplication[$e]";

/// `$XDG_CONFIG_HOME/kdeglobals`.
#[must_use]
pub fn path(xdg: &XdgDirs) -> PathBuf {
    xdg.config_home.join(FILE)
}

/// True when the session is KDE Plasma.
#[must_use]
pub fn is_kde(xdg: &XdgDirs) -> bool {
    xdg.current_desktops
        .iter()
        .any(|desktop| desktop.eq_ignore_ascii_case("KDE"))
}

/// The value of `BrowserApplication` as stored, or `None` when `kdeglobals`
/// is missing or has no such key.
#[must_use]
pub fn browser(xdg: &XdgDirs) -> Option<String> {
    let text = std::fs::read_to_string(path(xdg)).ok()?;
    stored_value(&text).map(|(_, value)| value)
}

/// The key that holds the value, and the value.
fn stored_value(text: &str) -> Option<(&'static str, String)> {
    let groups = keyfile::parse(text);
    groups
        .iter()
        .filter(|group| group.name == GROUP)
        .find_map(|group| {
            [BROWSER_KEY, BROWSER_KEY_EXPANDED]
                .into_iter()
                .find_map(|key| {
                    let value = group.get(key)?.trim().to_owned();
                    (!value.is_empty()).then_some((key, value))
                })
        })
}

/// What [`set_browser`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// The session is not KDE; nothing was touched.
    NotKde,
    /// `kdeglobals` already names Wye; nothing was written, and the caller's
    /// remembered previous value stays as it is.
    AlreadySet,
    /// Wye is now the browser. `previous` is what `kdeglobals` held before,
    /// as stored, for [`restore_browser`].
    Set { previous: Option<String> },
}

/// Sets `BrowserApplication` to `id` on KDE (DEF-02).
///
/// # Errors
///
/// Returns [`DefaultBrowserError::Managed`] when `kdeglobals` is a symlink or
/// read-only, and [`DefaultBrowserError::Io`] when reading or writing fails.
pub fn set_browser(xdg: &XdgDirs, id: &DesktopId) -> Result<Applied, DefaultBrowserError> {
    if !is_kde(xdg) {
        return Ok(Applied::NotKde);
    }
    let path = path(xdg);
    let text = read_existing(&path)?.unwrap_or_default();
    let current = stored_value(&text);
    if current
        .as_ref()
        .is_some_and(|(_, value)| value == id.as_str())
    {
        return Ok(Applied::AlreadySet);
    }
    ensure_writable(&path)?;
    let key = current.as_ref().map_or(BROWSER_KEY, |(key, _)| *key);
    match xdg.find_program(KWRITECONFIG) {
        Some(program) => kwriteconfig(&program, &path, Some(id.as_str()))?,
        None => write(&path, &set_keys(&text, GROUP, &[(key, id.as_str())]))?,
    }
    Ok(Applied::Set {
        previous: current.map(|(_, value)| value),
    })
}

/// What [`restore_browser`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restored {
    NotKde,
    /// `kdeglobals` no longer names Wye (the user or another browser changed
    /// it since); it was left alone.
    NotOurs,
    /// The previous value is back, or the key was removed when there was
    /// none.
    Restored,
}

/// Undoes [`set_browser`] (DEF-05): puts `previous` back as it was stored, or
/// removes the key when there was none, but only while `kdeglobals` still
/// names `wye`, so a browser the user chose in the meantime is not undone.
///
/// # Errors
///
/// Returns [`DefaultBrowserError::Managed`] when `kdeglobals` is a symlink or
/// read-only, and [`DefaultBrowserError::Io`] when reading or writing fails.
pub fn restore_browser(
    xdg: &XdgDirs,
    wye: &DesktopId,
    previous: Option<&str>,
) -> Result<Restored, DefaultBrowserError> {
    if !is_kde(xdg) {
        return Ok(Restored::NotKde);
    }
    let path = path(xdg);
    let Some(text) = read_existing(&path)? else {
        return Ok(Restored::NotOurs);
    };
    let Some((key, value)) = stored_value(&text) else {
        return Ok(Restored::NotOurs);
    };
    if value != wye.as_str() {
        return Ok(Restored::NotOurs);
    }
    ensure_writable(&path)?;
    let previous = previous.map(str::trim).filter(|value| !value.is_empty());
    if let Some(program) = xdg.find_program(KWRITECONFIG) {
        kwriteconfig(&program, &path, previous)?;
        return Ok(Restored::Restored);
    }
    let updated = match previous {
        Some(previous) => set_keys(&text, GROUP, &[(key, previous)]),
        None => remove_key(&text, GROUP, key),
    };
    write(&path, &updated)?;
    Ok(Restored::Restored)
}

/// Set `BrowserApplication` in `path` to `value`, or delete it for `None`,
/// with `kwriteconfig6` at `program`.
fn kwriteconfig(
    program: &Path,
    path: &Path,
    value: Option<&str>,
) -> Result<(), DefaultBrowserError> {
    let mut command = Command::new(program);
    command
        .arg("--file")
        .arg(path)
        .args(["--group", GROUP, "--key", BROWSER_KEY]);
    match value {
        Some(value) => command.arg("--").arg(value),
        None => command.arg("--delete"),
    };
    let failed = |source: io::Error| DefaultBrowserError::Io {
        path: path.to_path_buf(),
        source,
    };
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .map_err(failed)?;
    if output.status.success() {
        return Ok(());
    }
    Err(failed(io::Error::other(format!(
        "{KWRITECONFIG} failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))))
}

fn write(path: &Path, text: &str) -> Result<(), DefaultBrowserError> {
    atomic::write(path, text.as_bytes(), Some(path)).map_err(|source| DefaultBrowserError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests;
