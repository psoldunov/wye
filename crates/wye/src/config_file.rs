//! Reading `config.toml` (CFG-01).
//!
//! A broken configuration must never stop links opening: every problem is
//! reported and Wye carries on with what it could read, or with the
//! defaults.

use std::io;
use std::path::Path;

use wye_core::config::ConfigError;
use wye_core::{Config, Loaded, ServiceCatalogue};

/// The outcome of reading the configuration file.
#[derive(Debug)]
pub enum Read {
    /// There is no file; the defaults apply.
    Missing,
    /// The file parsed, possibly with warnings.
    Parsed(Box<Loaded>),
    /// The file is not valid TOML or has the wrong shape.
    Invalid(ConfigError),
    /// The file exists but could not be read.
    Unreadable(io::Error),
}

/// Reads and parses the file at `path`.
pub fn read(path: &Path) -> Read {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Read::Missing,
        Err(error) => Read::Unreadable(error),
    }
}

fn parse(text: &str) -> Read {
    let catalogue = ServiceCatalogue::shipped();
    let known: Vec<&str> = catalogue
        .services()
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    match Config::parse(text, &known) {
        Ok(loaded) => Read::Parsed(Box::new(loaded)),
        Err(error) => Read::Invalid(error),
    }
}

/// Loads the configuration for routing. Errors are reported to `err` and
/// the defaults used instead; warnings are reported only when `warn` is set.
///
/// # Errors
///
/// Returns an error only when writing to `err` fails.
pub fn load(path: &Path, err: &mut dyn io::Write, warn: bool) -> io::Result<Config> {
    let shown = path.display();
    Ok(match read(path) {
        Read::Missing => Config::default(),
        Read::Parsed(loaded) => {
            if warn {
                for warning in &loaded.warnings {
                    writeln!(err, "wye: {shown}: {warning}")?;
                }
            }
            loaded.config
        }
        Read::Invalid(error) => {
            writeln!(err, "wye: {shown}: {error}; using defaults")?;
            Config::default()
        }
        Read::Unreadable(error) => {
            writeln!(err, "wye: {shown}: {error}; using defaults")?;
            Config::default()
        }
    })
}

#[cfg(test)]
mod tests {
    use wye_core::Target;

    use super::*;

    #[test]
    fn missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(read(&dir.path().join("none.toml")), Read::Missing));
        let mut err = Vec::new();
        let config = load(&dir.path().join("none.toml"), &mut err, true).unwrap();
        assert_eq!(config, Config::default());
        assert!(err.is_empty());
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[browsers\n").unwrap();
        assert!(matches!(read(&path), Read::Invalid(_)));
        let mut err = Vec::new();
        assert_eq!(load(&path, &mut err, false).unwrap(), Config::default());
        let err = String::from_utf8(err).unwrap();
        assert!(err.ends_with("; using defaults\n"), "{err}");
    }

    #[test]
    fn warnings_are_collected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "unknown-key = 1\n").unwrap();
        let Read::Parsed(loaded) = read(&path) else {
            panic!("expected a parsed file");
        };
        assert_eq!(loaded.warnings.len(), 1);
        let mut quiet = Vec::new();
        load(&path, &mut quiet, false).unwrap();
        assert!(quiet.is_empty());
        let mut loud = Vec::new();
        load(&path, &mut loud, true).unwrap();
        assert!(String::from_utf8(loud).unwrap().contains("unknown-key"));
    }

    #[test]
    fn parsed_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[browsers]\nprimary = { app = \"a.desktop\" }\n").unwrap();
        let config = load(&path, &mut Vec::new(), false).unwrap();
        assert_eq!(
            config.browsers.primary,
            Target::App(wye_core::DesktopId::new("a.desktop").unwrap())
        );
    }
}
