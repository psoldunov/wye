//! `GetHistory`: recent links (DLG-HIS-01 to DLG-HIS-04, TRAY-15, PIPE-16).

use serde::{Deserialize, Serialize};

use crate::TargetSpec;
use crate::context::Entry;

/// The history window's contents.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct History {
    /// History is switched on (ADV-09).
    pub enabled: bool,
    /// Entries, newest first, at most 100.
    pub entries: Vec<HistoryEntry>,
}

/// One routed link (12-data-model.md, `HistoryEntry`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryEntry {
    /// Stable ID for `DeleteHistoryEntry` and `ReopenHistoryEntry`.
    pub id: u64,
    /// When, in Unix seconds.
    pub time: i64,
    /// The link as received.
    pub original_url: String,
    /// The link as opened.
    pub final_url: String,
    /// How the link arrived.
    pub entry: Entry,
    /// Source app desktop ID or executable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Source app display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// The target it opened in.
    pub target: TargetSpec,
    /// Display name of the target.
    pub target_name: String,
    /// Icon name or path of the target's app, for a target `GetTargets`
    /// does not list, such as an installed app the configuration no longer
    /// names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_icon: Option<String>,
    /// Why: the matched rule or web app mapping, or the picker.
    pub reason: String,
    /// Tracking parameters were removed.
    pub cleaned: bool,
    /// A wrapper or short link was expanded.
    pub expanded: bool,
}
