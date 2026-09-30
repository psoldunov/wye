//! `RunScript`: the script editor's test run (SCR-04).

use serde::{Deserialize, Serialize};

/// The result of running a transform script on one URL.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptRun {
    /// The script ran and returned a valid result.
    pub ok: bool,
    /// The URL it returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Changed character ranges of `url`, `[start, end)` in UTF-16 units, for
    /// highlighting.
    pub changed: Vec<[u32; 2]>,
    /// What went wrong.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Line of the error, 1-based.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Run time in microseconds.
    pub micros: u64,
    /// `console.log` output, one entry per call.
    pub logs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_ranges_are_pairs() {
        let run = ScriptRun {
            ok: true,
            url: Some("https://example.com/".into()),
            changed: vec![[8, 19]],
            ..ScriptRun::default()
        };
        let json = serde_json::to_value(&run).expect("encodes");
        assert_eq!(json["changed"], serde_json::json!([[8, 19]]));
        assert!(json.get("error").is_none());
    }
}
