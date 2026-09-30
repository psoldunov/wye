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
    /// The session's message locale (`$LC_ALL`, `$LC_MESSAGES`, `$LANG`),
    /// which picks localised desktop-entry keys (DISC-03).
    pub locale: Locale,
}

/// A message locale such as `de_AT.UTF-8@euro`, reduced to the parts the
/// Desktop Entry specification's key lookup uses: language, country and
/// modifier. The encoding is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Locale {
    lang: String,
    country: Option<String>,
    modifier: Option<String>,
}

impl Locale {
    /// The locale that looks up only unlocalised keys (`C`, `POSIX`).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Parses `lang[_COUNTRY][.ENCODING][@MODIFIER]`. `C`, `POSIX` and
    /// empty text have no language to look up, so they give `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (rest, modifier) = match text.split_once('@') {
            Some((rest, modifier)) => (rest, Some(modifier)),
            None => (text, None),
        };
        let rest = rest.split_once('.').map_or(rest, |(before, _)| before);
        let (lang, country) = match rest.split_once('_') {
            Some((lang, country)) => (lang, Some(country)),
            None => (rest, None),
        };
        let part = |value: &str| (!value.is_empty()).then(|| value.to_owned());
        if lang.is_empty() || lang == "C" || lang == "POSIX" {
            return None;
        }
        Some(Self {
            lang: lang.to_owned(),
            country: country.and_then(part),
            modifier: modifier.and_then(part),
        })
    }

    /// The locale an environment selects: the first of `LC_ALL`,
    /// `LC_MESSAGES` and `LANG` that is set and not empty decides, as in
    /// `setlocale`; when that one is `C` or `POSIX`, nothing is localised.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| lookup(name))
            .map(|value| value.to_string_lossy().into_owned())
            .find(|value| !value.trim().is_empty())
            .and_then(|value| Self::parse(&value))
            .unwrap_or_default()
    }

    /// The key suffixes to try, best first, in the order the Desktop Entry
    /// specification gives: `lang_COUNTRY@MODIFIER`, `lang_COUNTRY`,
    /// `lang@MODIFIER`, `lang`. Suffixes the locale has no parts for are
    /// left out. `Name[de_AT]` is tried as `de_AT`, never as `de-AT`.
    #[must_use]
    pub fn candidates(&self) -> Vec<String> {
        if self.lang.is_empty() {
            return Vec::new();
        }
        let lang = &self.lang;
        let mut out = Vec::with_capacity(4);
        if let (Some(country), Some(modifier)) = (&self.country, &self.modifier) {
            out.push(format!("{lang}_{country}@{modifier}"));
        }
        if let Some(country) = &self.country {
            out.push(format!("{lang}_{country}"));
        }
        if let Some(modifier) = &self.modifier {
            out.push(format!("{lang}@{modifier}"));
        }
        out.push(lang.clone());
        out
    }
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
        let locale = Locale::from_lookup(&lookup);

        Ok(Self {
            config_home: single("XDG_CONFIG_HOME", home.join(".config")),
            config_dirs: list("XDG_CONFIG_DIRS", &["/etc/xdg"]),
            data_home: single("XDG_DATA_HOME", home.join(".local/share")),
            data_dirs: list("XDG_DATA_DIRS", &["/usr/local/share", "/usr/share"]),
            current_desktops,
            search_path,
            locale,
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
    fn parses_locales() {
        let parse = |text: &str| Locale::parse(text).map(|l| l.candidates());
        assert_eq!(
            parse("de_AT.UTF-8@euro"),
            Some(vec![
                "de_AT@euro".to_owned(),
                "de_AT".to_owned(),
                "de@euro".to_owned(),
                "de".to_owned()
            ])
        );
        assert_eq!(
            parse("de_AT"),
            Some(vec!["de_AT".to_owned(), "de".to_owned()])
        );
        assert_eq!(
            parse("sr@latin"),
            Some(vec!["sr@latin".to_owned(), "sr".to_owned()])
        );
        assert_eq!(parse("fr.UTF-8"), Some(vec!["fr".to_owned()]));
        assert_eq!(
            parse("pt_BR.utf8"),
            Some(vec!["pt_BR".to_owned(), "pt".to_owned()])
        );
        assert_eq!(parse("C"), None);
        assert_eq!(parse("POSIX"), None);
        assert_eq!(parse("C.UTF-8"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("_DE"), None);
        assert!(Locale::none().candidates().is_empty());
    }

    #[test]
    fn the_locale_comes_from_lc_all_then_lc_messages_then_lang() {
        let locale = |vars: &[(&str, &str)]| {
            let mut all = vec![("HOME", "/home/u")];
            all.extend_from_slice(vars);
            XdgDirs::from_lookup(lookup(&all)).unwrap().locale
        };
        assert_eq!(locale(&[]), Locale::none());
        assert_eq!(
            locale(&[("LANG", "de_DE.UTF-8")]),
            Locale::parse("de_DE").unwrap()
        );
        assert_eq!(
            locale(&[("LANG", "de_DE.UTF-8"), ("LC_MESSAGES", "fr_FR.UTF-8")]),
            Locale::parse("fr_FR").unwrap()
        );
        assert_eq!(
            locale(&[
                ("LANG", "de_DE.UTF-8"),
                ("LC_MESSAGES", "fr_FR.UTF-8"),
                ("LC_ALL", "es_ES.UTF-8")
            ]),
            Locale::parse("es_ES").unwrap()
        );
        // An empty LC_ALL is unset; a C LC_ALL wins and switches localisation off.
        assert_eq!(
            locale(&[("LC_ALL", ""), ("LANG", "de_DE.UTF-8")]),
            Locale::parse("de_DE").unwrap()
        );
        assert_eq!(
            locale(&[("LC_ALL", "C"), ("LANG", "de_DE.UTF-8")]),
            Locale::none()
        );
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
