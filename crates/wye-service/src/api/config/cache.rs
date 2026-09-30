//! Reading `config.toml` into the configuration the service keeps (SET-06,
//! risk 16). Blocking: runs on a blocking thread.
//!
//! A missing file is the defaults. A file that cannot be read or is not
//! TOML keeps the last good configuration and records why, so a half-saved
//! hand edit never changes how links are routed; the CLI, which has no last
//! good configuration, still falls back to the defaults.

use std::io;
use std::path::Path;
use std::sync::Arc;

use wye_core::{Config, ConfigWarning, Loaded, Pipeline, ServiceCatalogue};

use crate::api::link::Environment;

/// The configuration in use and the health of the file behind it.
#[derive(Debug, Clone)]
pub(crate) struct Current {
    /// The files it was read from.
    pub environment: Arc<Environment>,
    /// The configuration as read, for `GetConfig` and as the base of every
    /// patch.
    pub config: Config,
    /// The corrected copy links are routed with.
    pub pipeline: Arc<Pipeline>,
    /// Problems found in the file.
    pub warnings: Vec<ConfigWarning>,
    /// Saving keeps every value the file set ([`Loaded::is_lossless`]).
    pub lossless: bool,
    /// The file may be replaced: not a symlink (home-manager), not
    /// read-only.
    pub writable: bool,
    /// Why the file on disk is not in use (it is broken; the last good
    /// configuration is).
    pub error: Option<String>,
    /// Bumps on every change (`ConfigRevision`); 0 is never used, so a
    /// caller's 0 always means "skip the check".
    pub revision: u64,
}

impl Current {
    /// Whether `other` says the same: same files, configuration and health.
    /// The revision does not count.
    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.environment, &other.environment)
            && self.config == other.config
            && self.warnings == other.warnings
            && self.writable == other.writable
            && self.error == other.error
    }

    /// The warnings, one line each, for `Status`.
    pub fn warning_lines(&self) -> Vec<String> {
        self.warnings.iter().map(ToString::to_string).collect()
    }
}

/// The IDs of the shipped web app catalogue, which the parser checks
/// mappings against.
pub(crate) fn known_services(catalogue: &ServiceCatalogue) -> Vec<&str> {
    catalogue
        .services()
        .iter()
        .map(|service| service.id.as_str())
        .collect()
}

/// Parse `text` as a configuration file.
///
/// # Errors
///
/// The parser's message when `text` is not TOML.
pub(crate) fn parse(text: &str) -> Result<Loaded, String> {
    let catalogue = ServiceCatalogue::shipped();
    Config::parse(text, &known_services(&catalogue)).map_err(|error| error.to_string())
}

/// Read the configuration `environment` names. `last_good` is what to keep
/// when the file is broken; without one, the defaults.
pub(crate) fn load(environment: Arc<Environment>, last_good: Option<&Current>) -> Current {
    let path = environment.config.clone();
    let writable = is_writable(&path);
    let read = match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Loaded {
            config: Config::default(),
            warnings: Vec::new(),
        }),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    };
    match read {
        Ok(loaded) => from_loaded(environment, loaded, writable, 0),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "configuration not usable; keeping the last good one");
            // A broken `last_good` already holds the configuration before it.
            let kept = last_good.map_or_else(Config::default, |good| good.config.clone());
            Current {
                error: Some(error),
                // Writing now would replace what the user is editing.
                lossless: false,
                ..from_loaded(
                    environment,
                    Loaded {
                        config: kept,
                        warnings: Vec::new(),
                    },
                    writable,
                    0,
                )
            }
        }
    }
}

/// A [`Current`] for a successfully read configuration.
pub(crate) fn from_loaded(
    environment: Arc<Environment>,
    loaded: Loaded,
    writable: bool,
    revision: u64,
) -> Current {
    let lossless = loaded.is_lossless();
    Current {
        environment,
        pipeline: Arc::new(Pipeline::with_shipped_data(loaded.config.clone())),
        config: loaded.config,
        warnings: loaded.warnings,
        lossless,
        writable,
        error: None,
        revision,
    }
}

/// Whether Wye may replace the file at `path`: a symlink belongs to whatever
/// made it (home-manager links into the read-only Nix store) and a read-only
/// file was made read-only on purpose. A missing file can be created.
pub(crate) fn is_writable(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => !meta.file_type().is_symlink() && !meta.permissions().readonly(),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn environment(dir: &Path) -> Arc<Environment> {
        let home = dir.to_owned();
        let environment = Environment::from_lookup(move |name| {
            (name == "HOME").then(|| OsString::from(home.clone()))
        })
        .expect("home");
        Arc::new(environment)
    }

    fn write(environment: &Environment, text: &str) {
        let path = &environment.config;
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("written");
    }

    #[test]
    fn a_missing_file_is_the_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let current = load(environment(dir.path()), None);
        assert_eq!(current.config, Config::default());
        assert!(current.writable && current.lossless && current.error.is_none());
    }

    #[test]
    fn a_broken_file_keeps_the_last_good_configuration() {
        let dir = tempfile::tempdir().expect("temp dir");
        let environment = environment(dir.path());
        write(&environment, "[general]\nlaunch-at-login = false\n");
        let good = load(environment.clone(), None);
        assert!(!good.config.general.launch_at_login);

        write(&environment, "[general\n");
        let broken = load(environment, Some(&good));
        assert!(broken.error.is_some(), "risk 16: the error is reported");
        assert_eq!(broken.config, good.config, "risk 16: last good kept");
        assert!(!broken.lossless, "a broken file is never overwritten");
    }

    #[test]
    fn unknown_keys_make_the_file_lossy() {
        let dir = tempfile::tempdir().expect("temp dir");
        let environment = environment(dir.path());
        write(&environment, "surprise = 1\n");
        let current = load(environment, None);
        assert!(!current.lossless);
        assert_eq!(current.warning_lines().len(), 1);
    }

    #[test]
    fn a_symlinked_file_is_not_writable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let environment = environment(dir.path());
        let real = dir.path().join("store.toml");
        std::fs::write(&real, "").expect("written");
        std::fs::create_dir_all(environment.config.parent().expect("parent")).expect("dir");
        std::os::unix::fs::symlink(&real, &environment.config).expect("linked");
        assert!(!load(environment, None).writable);
    }
}
