//! Reading and setting the default web browser in `mimeapps.list`
//! (DEF-02, DEF-03, DEF-07; [Association between MIME types and applications]).
//!
//! [Association between MIME types and applications]: https://specifications.freedesktop.org/mime-apps-spec/latest/

use std::fs;
use std::path::{Path, PathBuf};

use wye_core::DesktopId;

use crate::discovery::find_entry;
use crate::xdg::XdgDirs;
use crate::{atomic, keyfile};

const DEFAULTS_GROUP: &str = "Default Applications";
const MIMEAPPS: &str = "mimeapps.list";
const HTTP: &str = "x-scheme-handler/http";
const HTTPS: &str = "x-scheme-handler/https";
/// Registered only when the user opts in (DEF-07).
const HTML_TYPES: [&str; 2] = ["text/html", "application/xhtml+xml"];

/// Why the default could not be set.
#[derive(Debug, thiserror::Error)]
pub enum DefaultBrowserError {
    /// The file is a symlink (home-manager points it into the read-only Nix
    /// store) or read-only; whatever manages it has to change the default.
    #[error(
        "{path} is managed elsewhere (a symlink or read-only file); change the default browser there"
    )]
    Managed { path: PathBuf },
    #[error("cannot update {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Every `mimeapps.list` in lookup order, most important first: for each of
/// `$XDG_CONFIG_HOME`, `$XDG_CONFIG_DIRS`, `$XDG_DATA_HOME/applications` and
/// `$XDG_DATA_DIRS/*/applications`, the desktop-specific files
/// (`<desktop>-mimeapps.list`, lowercase) and then `mimeapps.list`.
#[must_use]
pub fn lookup_files(xdg: &XdgDirs) -> Vec<PathBuf> {
    let desktops: Vec<String> = xdg
        .current_desktops
        .iter()
        .map(|name| name.to_lowercase())
        .collect();
    let dirs = std::iter::once(xdg.config_home.clone())
        .chain(xdg.config_dirs.iter().cloned())
        .chain(xdg.applications_dirs());
    dirs.flat_map(|dir| {
        desktops
            .iter()
            .map(|desktop| dir.join(format!("{desktop}-{MIMEAPPS}")))
            .chain(std::iter::once(dir.join(MIMEAPPS)))
            .collect::<Vec<_>>()
    })
    .collect()
}

/// The default handler for `mime`: the first installed ID listed under
/// `[Default Applications]`, trying the files in lookup order. As the
/// mime-apps specification says, an ID whose desktop entry cannot be found
/// (or is `Hidden`) is skipped, so `removed.desktop;firefox.desktop;`
/// names Firefox; a file listing only missing apps defers to the next.
#[must_use]
pub fn default_for(xdg: &XdgDirs, mime: &str) -> Option<DesktopId> {
    lookup_files(xdg).iter().find_map(|path| {
        let text = fs::read_to_string(path).ok()?;
        keyfile::parse(&text)
            .iter()
            .filter(|group| group.name == DEFAULTS_GROUP)
            .find_map(|group| group.get(mime).map(keyfile::unescape_list))?
            .into_iter()
            .filter_map(|id| DesktopId::new(id).ok())
            .find(|id| find_entry(xdg, id).is_some())
    })
}

/// The current default web browser (the `https` handler).
#[must_use]
pub fn current_default(xdg: &XdgDirs) -> Option<DesktopId> {
    default_for(xdg, HTTPS)
}

/// True when `id` handles both `http` and `https` links.
#[must_use]
pub fn is_default(xdg: &XdgDirs, id: &DesktopId) -> bool {
    [HTTP, HTTPS]
        .iter()
        .all(|mime| default_for(xdg, mime).as_ref() == Some(id))
}

/// Makes `id` the default browser (DEF-02): sets `http` and `https` (plus
/// the HTML types when `include_html`, DEF-07) in
/// `$XDG_CONFIG_HOME/mimeapps.list`, and the same keys in any existing
/// `$XDG_CONFIG_HOME/<desktop>-mimeapps.list` that sets them, since that file
/// would shadow ours. Every other line is kept byte for byte; files are
/// replaced atomically.
///
/// # Errors
///
/// Returns [`DefaultBrowserError::Managed`] when a file to change is a
/// symlink or read-only, and [`DefaultBrowserError::Io`] when reading or
/// writing fails.
pub fn set_default(
    xdg: &XdgDirs,
    id: &DesktopId,
    include_html: bool,
) -> Result<(), DefaultBrowserError> {
    let mut mimes = vec![HTTP, HTTPS];
    if include_html {
        mimes.extend(HTML_TYPES);
    }
    let main = xdg.config_home.join(MIMEAPPS);
    let mut edits = vec![(main.clone(), mimes.clone())];
    for desktop in &xdg.current_desktops {
        let path = xdg
            .config_home
            .join(format!("{}-{MIMEAPPS}", desktop.to_lowercase()));
        let Some(text) = read_existing(&path)? else {
            continue;
        };
        let shadowed = set_keys_in(&text, &mimes);
        if !shadowed.is_empty() {
            edits.push((path, shadowed));
        }
    }
    // Check every file before writing any, so a managed file fails the whole
    // change instead of leaving it half applied.
    for (path, _) in &edits {
        ensure_writable(path)?;
    }
    for (path, keys) in edits {
        let text = read_existing(&path)?.unwrap_or_default();
        let pairs: Vec<(&str, &str)> = keys.iter().map(|mime| (*mime, id.as_str())).collect();
        let text = set_keys(&text, DEFAULTS_GROUP, &pairs);
        atomic::write(&path, text.as_bytes(), Some(&path))
            .map_err(|source| DefaultBrowserError::Io { path, source })?;
    }
    Ok(())
}

/// The `mimes` that `text` sets under `[Default Applications]`.
fn set_keys_in<'a>(text: &str, mimes: &[&'a str]) -> Vec<&'a str> {
    let groups = keyfile::parse(text);
    mimes
        .iter()
        .copied()
        .filter(|mime| {
            groups
                .iter()
                .any(|group| group.name == DEFAULTS_GROUP && group.get(mime).is_some())
        })
        .collect()
}

/// Sets `key=value` pairs in `group`, replacing existing keys in place and
/// adding missing ones after the group's last entry. The group is appended
/// when missing. All other lines are kept unchanged.
#[must_use]
pub fn set_keys(text: &str, group: &str, pairs: &[(&str, &str)]) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + pairs.len() + 2);
    let mut written = vec![false; pairs.len()];
    let mut in_group = false;
    let mut insert_at = None;
    for line in &lines {
        let trimmed = line.trim();
        if let Some(name) = keyfile::group_header(trimmed) {
            in_group = name == group;
            if in_group {
                insert_at = Some(out.len() + 1);
            }
        } else if in_group && !trimmed.is_empty() && !trimmed.starts_with('#') {
            let key = trimmed.split_once('=').map(|(key, _)| key.trim_end());
            let pair =
                key.and_then(|key| pairs.iter().zip(&mut written).find(|((k, _), _)| *k == key));
            if let Some(((key, value), done)) = pair {
                *done = true;
                let ending = if line.ends_with('\n') { "\n" } else { "" };
                out.push(format!("{key}={value}{ending}"));
                insert_at = Some(out.len());
                continue;
            }
            insert_at = Some(out.len() + 1);
        }
        out.push((*line).to_owned());
    }
    let missing: Vec<String> = pairs
        .iter()
        .zip(&written)
        .filter(|(_, done)| !**done)
        .map(|((key, value), _)| format!("{key}={value}\n"))
        .collect();
    if missing.is_empty() {
        return out.concat();
    }
    let at = insert_at.unwrap_or_else(|| {
        if out.last().is_some_and(|line| !line.ends_with('\n')) {
            out.push("\n".to_owned());
        }
        if !out.is_empty() {
            out.push("\n".to_owned());
        }
        out.push(format!("[{group}]\n"));
        out.len()
    });
    if let Some(previous) = at.checked_sub(1).and_then(|index| out.get_mut(index)) {
        if !previous.ends_with('\n') {
            previous.push('\n');
        }
    }
    let tail = out.split_off(at);
    out.into_iter().chain(missing).chain(tail).collect()
}

fn read_existing(path: &Path) -> Result<Option<String>, DefaultBrowserError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(DefaultBrowserError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn ensure_writable(path: &Path) -> Result<(), DefaultBrowserError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || meta.permissions().readonly() => {
            Err(DefaultBrowserError::Managed {
                path: path.to_path_buf(),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DefaultBrowserError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests;
