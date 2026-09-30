//! Where the service reads the configuration, the state and the installed
//! apps from, and reading them for one link.
//!
//! Read per request for now; a cached, watched configuration and inventory
//! replace [`Snapshot::load`] later (U07), which is why everything goes
//! through it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use wye_core::{Config, DesktopId, Pipeline, ServiceCatalogue};
use wye_desktop::{Inventory, WYE_DESKTOP_ID, XdgDirs};

/// The key in `state.toml` naming the browser Wye replaced as the default.
const PREVIOUS_DEFAULT: &str = "previous-default-browser";

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

/// Everything one link is routed with.
pub(crate) struct Snapshot {
    pub pipeline: Pipeline,
    pub inventory: Inventory,
    /// The browser Wye replaced as the default (for the picker stand-in).
    pub previous_default: Option<DesktopId>,
}

impl Snapshot {
    /// Read the configuration, the state and the installed apps. A missing
    /// or broken file never stops a link: it is logged and the defaults are
    /// used.
    pub fn load(environment: &Environment) -> Self {
        let wye = DesktopId::new(WYE_DESKTOP_ID).ok();
        let inventory = match &wye {
            Some(wye) => Inventory::scan(&environment.xdg, wye),
            None => Inventory::from_apps(Vec::new(), Vec::new()),
        };
        Self {
            pipeline: Pipeline::with_shipped_data(load_config(&environment.config)),
            inventory,
            previous_default: previous_default(&environment.state),
        }
    }
}

fn load_config(path: &Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "cannot read the configuration; using defaults");
            return Config::default();
        }
    };
    let catalogue = ServiceCatalogue::shipped();
    let known: Vec<&str> = catalogue
        .services()
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    match Config::parse(&text, &known) {
        Ok(loaded) => {
            for warning in &loaded.warnings {
                tracing::info!(path = %path.display(), %warning, "configuration warning");
            }
            loaded.config
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "invalid configuration; using defaults");
            Config::default()
        }
    }
}

/// `previous-default-browser` from `state.toml`, if readable.
fn previous_default(path: &Path) -> Option<DesktopId> {
    let text = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = toml::from_str(&text)
        .inspect_err(|error| tracing::warn!(path = %path.display(), %error, "invalid state file"))
        .ok()?;
    let id = table.get(PREVIOUS_DEFAULT)?.as_str()?;
    DesktopId::new(id).ok()
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn broken_files_fall_back_to_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "[browsers\n").expect("written");
        assert_eq!(load_config(&config), Config::default());
        assert_eq!(
            load_config(&dir.path().join("none.toml")),
            Config::default()
        );

        let state = dir.path().join("state.toml");
        std::fs::write(&state, "previous-default-browser = \"firefox.desktop\"\n")
            .expect("written");
        assert_eq!(
            previous_default(&state),
            DesktopId::new("firefox.desktop").ok()
        );
        std::fs::write(&state, "not toml [").expect("written");
        assert_eq!(previous_default(&state), None);
    }
}
