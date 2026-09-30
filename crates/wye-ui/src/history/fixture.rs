//! The self-test's stand-in for the service: `fixtures/history.json`
//! carries what `GetHistory` and `GetTargets` would have returned, and the
//! window shows it without a bus (`HistoryBackend.loadFixture`).

use serde::Deserialize;
use wye_api::history::History;
use wye_api::targets::TargetInventory;

use super::sync::{Known, Snapshot};

/// What a `fixture` case holds.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fixture {
    #[serde(default)]
    pub history: History,
    #[serde(default)]
    pub targets: TargetInventory,
}

impl Fixture {
    /// Read a fixture from its JSON text.
    ///
    /// # Errors
    ///
    /// The serde error, naming the field, when the text is not a fixture.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// What the window shows for this fixture.
    #[must_use]
    pub fn snapshot(self) -> Snapshot {
        Snapshot {
            history: self.history,
            targets: self.targets,
            known: Known::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::settings::fixture::case_fixtures;

    const FIXTURES: &str = include_str!("../../fixtures/history.json");

    /// The `fixture` of every case of the history fixture file.
    fn fixtures() -> Vec<Value> {
        case_fixtures(FIXTURES)
    }

    #[test]
    fn every_history_fixture_decodes_as_service_data() {
        let cases = fixtures();
        assert!(cases.len() >= 3, "on, off and empty need a case each");
        let failures: Vec<String> = cases
            .iter()
            .filter_map(|fixture| Fixture::parse(&fixture.to_string()).err())
            .map(|error| error.to_string())
            .collect();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn the_fixtures_cover_every_state_of_the_window() {
        // DLG-HIS-04: off, empty, and a list
        let snapshots: Vec<Snapshot> = fixtures()
            .iter()
            .map(|fixture| {
                Fixture::parse(&fixture.to_string())
                    .expect("fixture")
                    .snapshot()
            })
            .collect();
        assert!(snapshots.iter().any(|s| !s.history.enabled));
        assert!(
            snapshots
                .iter()
                .any(|s| s.history.enabled && s.history.entries.is_empty())
        );
        assert!(snapshots.iter().any(|s| !s.history.entries.is_empty()));
    }
}
