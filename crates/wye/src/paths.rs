//! Where Wye keeps its files ([12-data-model.md](../../../docs/spec/12-data-model.md)).
//!
//! The configuration is the user's (and may be a read-only file managed by
//! home-manager); internal state lives apart from it, under
//! `$XDG_STATE_HOME`.

use std::ffi::OsString;
use std::path::PathBuf;

use wye_desktop::XdgDirs;

/// Wye's files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `$XDG_CONFIG_HOME/wye/config.toml`.
    pub config: PathBuf,
    /// `$XDG_STATE_HOME/wye/state.toml`.
    pub state: PathBuf,
}

impl Paths {
    /// Builds the paths from the XDG directories and an environment lookup
    /// for `XDG_STATE_HOME`, which [`XdgDirs`] does not cover.
    pub fn new(xdg: &XdgDirs, lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        Self {
            config: xdg.config_home.join("wye").join("config.toml"),
            state: wye_desktop::state::path(
                &xdg.home,
                lookup("XDG_STATE_HOME").map(PathBuf::from).as_deref(),
            ),
        }
    }

    pub fn from_env(xdg: &XdgDirs) -> Self {
        Self::new(xdg, |name| std::env::var_os(name))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn paths(vars: &[(&str, &str)]) -> Paths {
        let lookup = |name: &str| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        };
        let xdg = XdgDirs::from_lookup(lookup).unwrap();
        Paths::new(&xdg, lookup)
    }

    #[test]
    fn defaults_under_home() {
        let paths = paths(&[("HOME", "/home/u")]);
        assert_eq!(paths.config, Path::new("/home/u/.config/wye/config.toml"));
        assert_eq!(
            paths.state,
            Path::new("/home/u/.local/state/wye/state.toml")
        );
    }

    #[test]
    fn follows_xdg_variables() {
        let paths = paths(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_STATE_HOME", "/s"),
        ]);
        assert_eq!(paths.config, Path::new("/c/wye/config.toml"));
        assert_eq!(paths.state, Path::new("/s/wye/state.toml"));
    }

    #[test]
    fn ignores_relative_state_home() {
        let paths = paths(&[("HOME", "/home/u"), ("XDG_STATE_HOME", "state")]);
        assert_eq!(
            paths.state,
            Path::new("/home/u/.local/state/wye/state.toml")
        );
    }
}
