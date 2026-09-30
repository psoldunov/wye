//! Launch at login (GEN-01): an XDG autostart entry,
//! `$XDG_CONFIG_HOME/autostart/dev.soldunov.wye.desktop`, that starts
//! `wye service --activate` with the session ([Desktop Application
//! Autostart Specification]).
//!
//! The caller passes the absolute path of the `wye` executable: the entry
//! must not depend on `$PATH`, which the session manager that reads it may
//! not share. A symlink at the entry's place (home-manager links
//! `xdg.configFile`) belongs to whatever made it, so it is never replaced or
//! removed ([`AutostartError::Managed`]).
//!
//! [Desktop Application Autostart Specification]: https://specifications.freedesktop.org/autostart-spec/latest/

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic;
use crate::keyfile;
use crate::launch::WYE_DESKTOP_ID;
use crate::xdg::XdgDirs;

const AUTOSTART_DIR: &str = "autostart";
/// The arguments after the executable path.
const SERVICE_ARGS: &str = "service --activate";

/// The entry could not be written or removed.
#[derive(Debug, thiserror::Error)]
pub enum AutostartError {
    /// The entry is a symlink or read-only; whatever manages it has to
    /// change it.
    #[error("{path} is managed elsewhere (a symlink or read-only file); change it there")]
    Managed { path: PathBuf },
    #[error("cannot update {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Where the autostart entry lives.
#[must_use]
pub fn entry_path(xdg: &XdgDirs) -> PathBuf {
    xdg.config_home.join(AUTOSTART_DIR).join(WYE_DESKTOP_ID)
}

/// The text of the autostart entry that starts `wye` (an absolute path).
#[must_use]
pub fn entry_text(wye: &Path) -> String {
    let exec = format!("{} {SERVICE_ARGS}", quote_exec(&wye.to_string_lossy()));
    let app_id = WYE_DESKTOP_ID.trim_end_matches(".desktop");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Wye\n\
         Comment=Sends every link to the right browser\n\
         Exec={exec}\n\
         Icon={app_id}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// Turns autostart on: writes the entry for `wye`, replacing an older one
/// atomically. Nothing is written when the entry already has this text.
///
/// # Errors
///
/// Returns [`AutostartError::Managed`] when the entry is a symlink or
/// read-only, and [`AutostartError::Io`] when writing fails.
pub fn enable(xdg: &XdgDirs, wye: &Path) -> Result<(), AutostartError> {
    let path = entry_path(xdg);
    let text = entry_text(wye);
    if fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    ensure_writable(&path)?;
    atomic::write(&path, text.as_bytes(), None)
        .map_err(|source| AutostartError::Io { path, source })
}

/// Turns autostart off: removes the entry. Returns whether there was one.
///
/// # Errors
///
/// Returns [`AutostartError::Managed`] when the entry is a symlink or
/// read-only, and [`AutostartError::Io`] when removing fails.
pub fn disable(xdg: &XdgDirs) -> Result<bool, AutostartError> {
    let path = entry_path(xdg);
    if fs::symlink_metadata(&path).is_err() {
        return Ok(false);
    }
    ensure_writable(&path)?;
    fs::remove_file(&path)
        .map(|()| true)
        .map_err(|source| AutostartError::Io { path, source })
}

/// Removes the entry only when Wye wrote it: a regular file (never a
/// symlink someone else made) whose `Exec` runs `wye service --activate`.
/// For a session whose login start is managed elsewhere (GEN-01). Returns
/// whether it was removed.
///
/// # Errors
///
/// Returns [`AutostartError::Io`] when reading or removing fails.
pub fn remove_own(xdg: &XdgDirs) -> Result<bool, AutostartError> {
    let path = entry_path(xdg);
    let io_error = |source| AutostartError::Io {
        path: path.clone(),
        source,
    };
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(io_error(source)),
    }
    let text = fs::read_to_string(&path).map_err(io_error)?;
    if !runs_wye(&text) {
        return Ok(false);
    }
    fs::remove_file(&path).map(|()| true).map_err(io_error)
}

/// Whether an entry's `Exec` is `<…/>wye service --activate`, as
/// [`entry_text`] writes it.
fn runs_wye(text: &str) -> bool {
    let groups = keyfile::parse(text);
    let Some(exec) = groups
        .iter()
        .find(|group| group.name == "Desktop Entry")
        .and_then(|group| group.get("Exec"))
    else {
        return false;
    };
    exec.trim()
        .strip_suffix(SERVICE_ARGS)
        .map(|program| program.trim().trim_matches('"'))
        .is_some_and(|program| {
            Path::new(program)
                .file_name()
                .is_some_and(|name| name == "wye")
        })
}

/// True when the entry exists and the session would run it: it is not
/// `Hidden` and `X-GNOME-Autostart-enabled` is not false.
#[must_use]
pub fn is_enabled(xdg: &XdgDirs) -> bool {
    let Ok(text) = fs::read_to_string(entry_path(xdg)) else {
        return false;
    };
    let groups = keyfile::parse(&text);
    let Some(main) = groups.iter().find(|group| group.name == "Desktop Entry") else {
        return false;
    };
    let off = |key: &str, value: &str| main.get(key).map(str::trim) == Some(value);
    !(off("Hidden", "true") || off("X-GNOME-Autostart-enabled", "false"))
}

fn ensure_writable(path: &Path) -> Result<(), AutostartError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || meta.permissions().readonly() => {
            Err(AutostartError::Managed {
                path: path.to_path_buf(),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(AutostartError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Quotes one `Exec` argument as the Desktop Entry Specification asks: an
/// argument with a reserved character goes in double quotes with `"`, `` ` ``,
/// `$` and `\` backslash-escaped, `%` is doubled, and every backslash is
/// doubled once more for the key file's own string escapes.
fn quote_exec(arg: &str) -> String {
    const RESERVED: &str = " \t\n\"'\\><~|&;$*?#()`";
    let argument = arg.replace('%', "%%");
    if !argument.chars().any(|c| RESERVED.contains(c)) {
        return argument;
    }
    let mut quoted = String::from("\"");
    for c in argument.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

#[cfg(test)]
mod tests;
