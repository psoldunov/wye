//! History (PIPE-16, ADV-09, DLG-HIS): the last 100 opened links, newest
//! first, as plain data. Reading and writing the file is the service's job;
//! this module keeps the ring, names the reasons and searches it.
//!
//! The file is JSON: `{ "version": 1, "entries": [ … ] }`, newest first.

use serde::{Deserialize, Serialize};

use crate::pipeline::{Decision, EntryPoint, Finished, LinkRequest, Resolution, Step};
use crate::target::Target;

/// How many links history keeps (ADV-09).
pub const CAPACITY: usize = 100;
/// The file format version.
pub const FORMAT_VERSION: u32 = 1;

/// Why a link went where it did (DLG-HIS-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Reason {
    /// The alternative-browser key was held, or the entry point asked for it
    /// (PIPE-06).
    AlternativeKey,
    Rule {
        name: String,
    },
    /// A web app mapping, by the service's name.
    Mapping {
        name: String,
    },
    /// No rule matched: the primary browser (PIPE-10).
    Fallback,
    /// The user chose in the picker (PIPE-13).
    Picker,
}

impl Reason {
    /// The text of the second line of a row: `rule “Meetings”` (DLG-HIS-02).
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::AlternativeKey => "alternative browser key".to_owned(),
            Self::Rule { name } => format!("rule “{name}”"),
            Self::Mapping { name } => format!("web app “{name}”"),
            Self::Fallback => "primary browser".to_owned(),
            Self::Picker => "picker choice".to_owned(),
        }
    }
}

/// One opened link (12-data-model.md, "`HistoryEntry`").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per thing that can change a link, shown as a badge"
)]
pub struct HistoryEntry {
    /// Unique within the history; assigned by [`History::record`].
    pub id: u64,
    /// Seconds since the Unix epoch, UTC.
    pub time: i64,
    /// The link as it arrived, for the row's tooltip (DLG-HIS-02).
    pub original: String,
    /// The link that was opened.
    pub url: String,
    pub entry: EntryPoint,
    /// The source app's desktop ID or executable, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target: Target,
    pub reason: Reason,
    #[serde(default)]
    pub expanded: bool,
    #[serde(default)]
    pub cleaned: bool,
    #[serde(default)]
    pub transformed: bool,
}

impl HistoryEntry {
    /// Describes an opened link from what the pipeline decided (PIPE-16).
    /// `time` is seconds since the Unix epoch. The ID is assigned when the
    /// entry is recorded.
    #[must_use]
    pub fn from_link(
        time: i64,
        request: &LinkRequest,
        resolution: &Resolution,
        finished: &Finished,
    ) -> Self {
        let changed = |wanted: fn(&Step) -> bool| finished.steps.iter().any(wanted);
        Self {
            id: 0,
            time,
            original: request.url.clone(),
            url: finished.url.to_string(),
            entry: request.entry,
            source: (!request.source.is_unknown()).then(|| request.source.to_string()),
            target: finished.target.clone(),
            reason: reason(resolution),
            expanded: changed(|s| {
                matches!(s, Step::Unwrapped { .. } | Step::ShortLinkExpanded { .. })
            }),
            cleaned: changed(|s| {
                matches!(s, Step::TrackingRemoved { .. } | Step::HttpsForced { .. })
            }),
            transformed: changed(|s| matches!(s, Step::Transformed { .. })),
        }
    }

    /// The badges of the row's second line: "expanded", "cleaned" and
    /// "transformed", for what changed the link (DLG-HIS-02).
    #[must_use]
    pub fn badges(&self) -> Vec<&'static str> {
        [
            (self.expanded, "expanded"),
            (self.cleaned, "cleaned"),
            (self.transformed, "transformed"),
        ]
        .into_iter()
        .filter_map(|(on, label)| on.then_some(label))
        .collect()
    }

    fn matches(&self, terms: &[String]) -> bool {
        let haystack = [
            self.url.as_str(),
            &self.original,
            self.source.as_deref().unwrap_or_default(),
            &self.target.to_string(),
            &self.reason.label(),
        ]
        .join("\n")
        .to_lowercase();
        terms.iter().all(|term| haystack.contains(term))
    }
}

fn reason(resolution: &Resolution) -> Reason {
    if resolution.target == Target::Picker {
        return Reason::Picker;
    }
    match &resolution.decision {
        Decision::AlternativeKey => Reason::AlternativeKey,
        Decision::Rule { name, .. } => Reason::Rule { name: name.clone() },
        Decision::Mapping { service } => Reason::Mapping {
            name: mapping_name(&resolution.steps).unwrap_or_else(|| service.clone()),
        },
        Decision::Fallback => Reason::Fallback,
    }
}

fn mapping_name(steps: &[Step]) -> Option<String> {
    steps.iter().find_map(|step| match step {
        Step::MappingMatched { service, .. } => Some(service.clone()),
        _ => None,
    })
}

/// A history file that cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("the history file is not valid: {0}")]
    Invalid(#[from] serde_json::Error),
    #[error("the history file has version {found}, this Wye reads version {FORMAT_VERSION}")]
    Version { found: u32 },
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    entries: Vec<HistoryEntry>,
}

/// The newest [`CAPACITY`] links, newest first.
///
/// Every change returns a new history; the old one stays as it was.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Newest first.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&HistoryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// The `count` newest entries (TRAY-15).
    #[must_use]
    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        self.entries.get(..count).unwrap_or(&self.entries)
    }

    /// Adds `entry` as the newest, giving it the next free ID, and drops the
    /// oldest beyond [`CAPACITY`]. IDs never repeat while the entries that
    /// used them are in the history.
    #[must_use]
    pub fn record(&self, entry: HistoryEntry) -> Self {
        let id = self
            .entries
            .iter()
            .map(|e| e.id)
            .max()
            .map_or(1, |max| max + 1);
        let entries = std::iter::once(HistoryEntry { id, ..entry })
            .chain(self.entries.iter().cloned())
            .take(CAPACITY)
            .collect();
        Self { entries }
    }

    /// Without the entry `id` (DLG-HIS-03, "Delete Entry").
    #[must_use]
    pub fn remove(&self, id: u64) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .filter(|entry| entry.id != id)
                .cloned()
                .collect(),
        }
    }

    /// An empty history (DLG-HIS-01, "Clear History").
    #[must_use]
    pub const fn cleared(&self) -> Self {
        Self::new()
    }

    /// The entries that match `query`, newest first (DLG-HIS-01). The query
    /// is split at spaces and every word must appear, in any case, in the
    /// link, the original link, the source app, the target or the reason. An
    /// empty query matches everything.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.entries
            .iter()
            .filter(|entry| entry.matches(&terms))
            .collect()
    }

    /// The JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error only if an entry cannot be written as JSON.
    pub fn to_json(&self) -> Result<String, HistoryError> {
        let file = File {
            version: FORMAT_VERSION,
            entries: self.entries.clone(),
        };
        Ok(serde_json::to_string_pretty(&file)?)
    }

    /// Reads the JSON file, keeping at most [`CAPACITY`] entries.
    ///
    /// # Errors
    ///
    /// Returns an error for text that is not a history file of this version.
    /// The caller decides whether to start over; history never stops a link.
    pub fn from_json(text: &str) -> Result<Self, HistoryError> {
        let file: File = serde_json::from_str(text)?;
        if file.version != FORMAT_VERSION {
            return Err(HistoryError::Version {
                found: file.version,
            });
        }
        Ok(Self {
            entries: file.entries.into_iter().take(CAPACITY).collect(),
        })
    }
}

#[cfg(test)]
mod tests;
