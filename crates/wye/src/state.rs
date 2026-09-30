//! Wye's internal state, kept apart from the configuration in
//! `$XDG_STATE_HOME/wye/state.toml` so a read-only `config.toml` (for
//! example one written by home-manager) never blocks it.

use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use wye_core::DesktopId;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct State {
    /// The default browser Wye replaced, restored by `wye default unset`
    /// (DEF-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_default_browser: Option<DesktopId>,
}

impl State {
    /// Reads the state file; a missing file is the empty state.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or parsed.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| format!("{} is not a valid state file", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    /// Writes the state atomically, creating its directory when needed.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory or file cannot be written.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = toml::to_string(self).context("cannot serialise the state")?;
        write_atomically(path, &text).with_context(|| format!("cannot write {}", path.display()))
    }
}

/// Writes `text` to a temporary file beside `path` and renames it over
/// `path`, so readers never see a half-written file.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(".tmp-{}", std::process::id()));
    let temp = Path::new(&temp);
    std::fs::write(temp, text)?;
    std::fs::rename(temp, path).inspect_err(|_| {
        // Best effort: the rename error is the one worth reporting.
        let _ = std::fs::remove_file(temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            State::load(&dir.path().join("state.toml")).unwrap(),
            State::default()
        );
    }

    #[test]
    fn round_trip_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/state.toml");
        let state = State {
            previous_default_browser: Some(DesktopId::new("firefox.desktop").unwrap()),
        };
        state.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "previous-default-browser = \"firefox.desktop\"\n");
        assert_eq!(State::load(&path).unwrap(), state);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn clearing_writes_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        State::default().save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), State::default());
    }

    #[test]
    fn invalid_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        std::fs::write(&path, "previous-default-browser = 3\n").unwrap();
        assert!(State::load(&path).is_err());
    }
}
