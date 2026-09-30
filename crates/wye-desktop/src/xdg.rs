//! XDG base directories and the other environment Wye reads its desktop
//! data from ([XDG Base Directory Specification]).
//!
//! Everything else in this crate takes an [`XdgDirs`] value instead of
//! reading the environment, so tests run against temporary directories.
//!
//! [XDG Base Directory Specification]: https://specifications.freedesktop.org/basedir-spec/latest/

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The directories and session facts desktop integration depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgDirs {
    /// `$HOME`.
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME`, default `~/.config`.
    pub config_home: PathBuf,
    /// `$XDG_CONFIG_DIRS`, default `/etc/xdg`, most important first.
    pub config_dirs: Vec<PathBuf>,
    /// `$XDG_DATA_HOME`, default `~/.local/share`.
    pub data_home: PathBuf,
    /// `$XDG_DATA_DIRS`, default `/usr/local/share:/usr/share`, most
    /// important first.
    pub data_dirs: Vec<PathBuf>,
    /// `$XDG_CURRENT_DESKTOP` split on `:`, for example `["GNOME"]`.
    pub current_desktops: Vec<String>,
    /// `$PATH`, used to resolve `TryExec` and bare executable names.
    pub search_path: Vec<PathBuf>,
}

/// The environment has no usable `$HOME`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("$HOME is not set to an absolute path")]
pub struct NoHomeError;

impl XdgDirs {
    /// Reads the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`NoHomeError`] when `$HOME` is unset or relative.
    pub fn from_env() -> Result<Self, NoHomeError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Builds the directories from an environment lookup, applying the
    /// specification's defaults. Relative paths in the variables are invalid
    /// and ignored, as the specification requires.
    ///
    /// # Errors
    ///
    /// Returns [`NoHomeError`] when `HOME` is missing or relative.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, NoHomeError> {
        let home = lookup("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or(NoHomeError)?;
        let single = |name: &str, default: PathBuf| {
            lookup(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or(default)
        };
        let list = |name: &str, default: &[&str]| {
            let parsed = lookup(name).map(|value| absolute_paths(&value));
            match parsed {
                Some(paths) if !paths.is_empty() => paths,
                _ => default.iter().map(PathBuf::from).collect(),
            }
        };
        let current_desktops = lookup("XDG_CURRENT_DESKTOP")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(':')
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let search_path = lookup("PATH")
            .map(|value| absolute_paths(&value))
            .unwrap_or_default();

        Ok(Self {
            config_home: single("XDG_CONFIG_HOME", home.join(".config")),
            config_dirs: list("XDG_CONFIG_DIRS", &["/etc/xdg"]),
            data_home: single("XDG_DATA_HOME", home.join(".local/share")),
            data_dirs: list("XDG_DATA_DIRS", &["/usr/local/share", "/usr/share"]),
            current_desktops,
            search_path,
            home,
        })
    }

    /// Every `applications` directory in precedence order: `$XDG_DATA_HOME`
    /// first, then each `$XDG_DATA_DIRS` entry, without duplicates (DISC-01).
    #[must_use]
    pub fn applications_dirs(&self) -> Vec<PathBuf> {
        let candidates = std::iter::once(&self.data_home)
            .chain(&self.data_dirs)
            .map(|dir| dir.join("applications"));
        let mut dirs: Vec<PathBuf> = Vec::new();
        for dir in candidates {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs
    }

    /// Resolves a program the way `TryExec` and `Exec` do: an absolute path
    /// must be an executable file; a bare name is looked up on `$PATH`.
    #[must_use]
    pub fn find_program(&self, program: &str) -> Option<PathBuf> {
        find_program(program, &self.search_path)
    }
}

/// See [`XdgDirs::find_program`].
#[must_use]
pub fn find_program(program: &str, search_path: &[PathBuf]) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    let path = Path::new(program);
    if path.is_absolute() {
        return is_executable(path).then(|| path.to_path_buf());
    }
    if program.contains('/') {
        return None;
    }
    search_path
        .iter()
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn absolute_paths(value: &OsString) -> Vec<PathBuf> {
    std::env::split_paths(value)
        .filter(|path| path.is_absolute())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use super::*;

    fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn applies_defaults() {
        let dirs = XdgDirs::from_lookup(lookup(&[("HOME", "/home/u")])).unwrap();
        assert_eq!(dirs.config_home, PathBuf::from("/home/u/.config"));
        assert_eq!(dirs.config_dirs, vec![PathBuf::from("/etc/xdg")]);
        assert_eq!(dirs.data_home, PathBuf::from("/home/u/.local/share"));
        assert_eq!(
            dirs.data_dirs,
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
        assert!(dirs.current_desktops.is_empty());
    }

    #[test]
    fn reads_variables_and_ignores_relative_paths() {
        let dirs = XdgDirs::from_lookup(lookup(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "relative"),
            ("XDG_DATA_HOME", "/data"),
            ("XDG_DATA_DIRS", "/a:rel:/b:/a"),
            ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
            ("PATH", "/bin:/usr/bin"),
        ]))
        .unwrap();
        assert_eq!(dirs.config_home, PathBuf::from("/home/u/.config"));
        assert_eq!(dirs.current_desktops, vec!["ubuntu", "GNOME"]);
        assert_eq!(
            dirs.applications_dirs(),
            vec![
                PathBuf::from("/data/applications"),
                PathBuf::from("/a/applications"),
                PathBuf::from("/b/applications"),
            ]
        );
        assert_eq!(dirs.search_path.len(), 2);
    }

    #[test]
    fn requires_home() {
        assert_eq!(XdgDirs::from_lookup(lookup(&[])), Err(NoHomeError));
        assert_eq!(
            XdgDirs::from_lookup(lookup(&[("HOME", "home")])),
            Err(NoHomeError)
        );
    }

    #[test]
    fn finds_programs() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        fs::write(&exe, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.path().join("data");
        fs::write(&plain, "").unwrap();
        let path = vec![dir.path().to_path_buf()];

        assert_eq!(find_program("tool", &path), Some(exe.clone()));
        assert_eq!(find_program(exe.to_str().unwrap(), &[]), Some(exe));
        assert_eq!(find_program("data", &path), None);
        assert_eq!(find_program("missing", &path), None);
        assert_eq!(find_program("sub/tool", &path), None);
    }
}
