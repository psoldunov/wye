//! The self-test's stand-in for the service: `fixtures/about.json` carries
//! what `Version` and `GetTroubleshooting` would have returned, and the
//! window shows it without a bus (`AboutBackend.loadFixture`).

use serde::Deserialize;

/// What a `fixture` case holds.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fixture {
    /// `Version`.
    pub version: String,
    /// `GetTroubleshooting`.
    pub troubleshooting: String,
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
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const FIXTURES: &str = include_str!("../../fixtures/about.json");

    #[test]
    fn every_about_fixture_decodes_as_service_data() {
        let value: Value = serde_json::from_str(FIXTURES).expect("valid JSON");
        let cases: Vec<&Value> = value["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .filter_map(|case| case["argument"].get("fixture"))
            .collect();
        assert!(!cases.is_empty(), "the about fixture has no data");
        for fixture in cases {
            let fixture = Fixture::parse(&fixture.to_string()).unwrap_or_else(|e| panic!("{e}"));
            // DLG-ABT-02: the section lists what was detected, one fact per line
            assert!(fixture.troubleshooting.contains("Held keys: "));
        }
    }
}
