//! The history file, `$XDG_STATE_HOME/wye/history.json` (decision 10,
//! PIPE-16): read once, rewritten atomically after every change. Blocking.
//!
//! History never stops a link: a file that cannot be read is logged and
//! history starts over.

use std::io;
use std::path::{Path, PathBuf};

use wye_api::Error;
use wye_core::history::History;

use crate::api::link::Environment;

/// The history file's name, next to `state.toml`.
const FILE: &str = "history.json";

/// Where the history lives for `environment`.
pub(crate) fn path(environment: &Environment) -> PathBuf {
    environment
        .state
        .parent()
        .map_or_else(|| PathBuf::from(FILE), |dir| dir.join(FILE))
}

/// The stored history; empty when the file is missing or unusable.
pub(crate) fn load(path: &Path) -> History {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return History::new(),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "cannot read the history; starting over");
            return History::new();
        }
    };
    History::from_json(&text).unwrap_or_else(|error| {
        tracing::warn!(path = %path.display(), %error, "unusable history; starting over");
        History::new()
    })
}

/// Replace the file with `history`.
///
/// # Errors
///
/// `Failed` when it cannot be written.
pub(crate) fn save(path: &Path, history: &History) -> Result<(), Error> {
    let text = history
        .to_json()
        .map_err(|error| Error::failed(format!("cannot encode the history: {error}")))?;
    wye_desktop::atomic::write(path, text.as_bytes(), Some(path))
        .map_err(|error| Error::failed(format!("cannot save {}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use wye_core::Target;
    use wye_core::history::{CAPACITY, HistoryEntry, Reason};
    use wye_core::pipeline::EntryPoint;

    use super::*;

    fn entry(url: &str) -> HistoryEntry {
        HistoryEntry {
            id: 0,
            time: 1,
            original: url.to_owned(),
            url: url.to_owned(),
            entry: EntryPoint::Handler,
            source: None,
            target: Target::Picker,
            reason: Reason::Picker,
            expanded: false,
            cleaned: false,
            transformed: false,
        }
    }

    #[test]
    fn the_ring_survives_a_restart() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("wye").join(FILE);
        let history = (0..=CAPACITY).fold(History::new(), |history, i| {
            history.record(entry(&format!("https://example.com/{i}")))
        });
        save(&path, &history).expect("saved");
        let loaded = load(&path);
        assert_eq!(loaded, history);
        assert_eq!(loaded.len(), CAPACITY, "ADV-09: the last 100");
        assert_eq!(
            loaded.entries()[0].url,
            format!("https://example.com/{CAPACITY}"),
            "newest first"
        );
    }

    #[test]
    fn a_broken_file_starts_over() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(FILE);
        std::fs::write(&path, "{").expect("written");
        assert!(load(&path).is_empty());
        assert!(load(&dir.path().join("none.json")).is_empty());
    }
}
