//! Where the service reads the configuration, the state and the installed
//! apps from, and what one link is routed with.

use std::ffi::OsString;
use std::path::PathBuf;

use wye_core::{DesktopId, Pipeline};
use wye_desktop::{Inventory, XdgDirs};

/// The files and directories the service works with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// XDG base directories, for desktop entries and browser profiles.
    pub xdg: XdgDirs,
    /// `config.toml`.
    pub config: PathBuf,
    /// `state.toml`.
    pub state: PathBuf,
    /// Where `/proc` is, for source-app detection.
    pub proc_root: PathBuf,
}

impl Environment {
    /// The session's directories, from the process environment.
    ///
    /// # Errors
    ///
    /// When the home directory cannot be found.
    pub fn from_env() -> Result<Self, wye_desktop::NoHomeError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// The directories `lookup` describes, with `/proc` as the process root.
    ///
    /// # Errors
    ///
    /// When `lookup` names no home directory.
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, wye_desktop::NoHomeError> {
        let xdg = XdgDirs::from_lookup(&lookup)?;
        let state_home = lookup("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| xdg.home.join(".local").join("state"));
        Ok(Self {
            config: xdg.config_home.join("wye").join("config.toml"),
            state: state_home.join("wye").join("state.toml"),
            proc_root: PathBuf::from("/proc"),
            xdg,
        })
    }
}

/// Everything one link is routed with, from the service's cached
/// configuration and inventory (`api::config::snapshot`).
pub(crate) struct Snapshot {
    pub pipeline: Pipeline,
    pub inventory: Inventory,
    /// The browser Wye replaced as the default (for the picker stand-in).
    pub previous_default: Option<DesktopId>,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn lookup(vars: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn paths_follow_the_xdg_variables() {
        let environment = Environment::from_lookup(lookup(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_STATE_HOME", "/s"),
        ]))
        .expect("home");
        assert_eq!(environment.config, Path::new("/c/wye/config.toml"));
        assert_eq!(environment.state, Path::new("/s/wye/state.toml"));
        assert_eq!(environment.proc_root, Path::new("/proc"));
    }

    #[test]
    fn a_relative_state_home_is_ignored() {
        let environment =
            Environment::from_lookup(lookup(&[("HOME", "/home/u"), ("XDG_STATE_HOME", "s")]))
                .expect("home");
        assert_eq!(
            environment.state,
            Path::new("/home/u/.local/state/wye/state.toml")
        );
    }
}
