//! What the history window's QML reads: one row per entry, built from the
//! service's `History` and target inventory (DLG-HIS-02, DLG-HIS-04).

use serde::Serialize;
use url::{Position, Url};
use wye_api::history::{History, HistoryEntry};
use wye_api::targets::TargetInventory;

use super::filter;
use crate::settings::icon;

/// The icon of a target the inventory does not know any more.
const FALLBACK_ICON: &str = "internet-web-browser";

/// Why a link went where it did, as a class for the badge's colour
/// (DLG-HIS-02). The label is the service's text; the class comes from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReasonKind {
    Rule,
    Mapping,
    Picker,
    Fallback,
    Alternative,
    Other,
}

impl ReasonKind {
    /// The class of the service's reason label (`wye_core::history::Reason::label`).
    #[must_use]
    pub fn of(label: &str) -> Self {
        if label.starts_with("rule ") {
            Self::Rule
        } else if label.starts_with("web app ") {
            Self::Mapping
        } else if label == "picker choice" {
            Self::Picker
        } else if label == "primary browser" {
            Self::Fallback
        } else if label == "alternative browser key" {
            Self::Alternative
        } else {
            Self::Other
        }
    }
}

/// One row of the list (DLG-HIS-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: u64,
    /// Unix seconds; QML formats it in the user's time zone.
    pub time: i64,
    /// The link's host, emphasised.
    pub host: String,
    /// The rest of the link, dimmed and middle-truncated.
    pub rest: String,
    pub final_url: String,
    pub original_url: String,
    /// The link was changed on its way (the tooltip shows the original).
    pub changed: bool,
    pub source: Option<String>,
    pub source_name: Option<String>,
    pub target_name: String,
    /// What `Kirigami.Icon.source` takes.
    pub icon: String,
    /// "Open in <target> Again" makes sense: the entry opened in a
    /// concrete target and not in the picker (DLG-HIS-03).
    pub same_target: bool,
    pub reason: String,
    pub reason_kind: ReasonKind,
    /// "cleaned" and "expanded", for what changed the link.
    pub badges: Vec<&'static str>,
}

/// The whole list for the window (DLG-HIS-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// History is switched on (ADV-09).
    pub enabled: bool,
    /// Entries the service holds, before the search.
    pub total: usize,
    /// The entries the search keeps, newest first.
    pub rows: Vec<Row>,
}

impl View {
    /// The window's list for `history`, narrowed by `query` (DLG-HIS-01).
    /// While history is off the window says so and lists nothing, though the
    /// service keeps what it had (DLG-HIS-04).
    #[must_use]
    pub fn build(history: &History, inventory: &TargetInventory, query: &str) -> Self {
        if !history.enabled {
            return Self {
                enabled: false,
                total: 0,
                rows: Vec::new(),
            };
        }
        let mut entries: Vec<&HistoryEntry> = filter::matching(&history.entries, query);
        // The service sends newest first; a stable sort keeps that for equal times.
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.time));
        Self {
            enabled: history.enabled,
            total: history.entries.len(),
            rows: entries
                .into_iter()
                .map(|entry| Row::of(entry, inventory))
                .collect(),
        }
    }
}

impl Row {
    fn of(entry: &HistoryEntry, inventory: &TargetInventory) -> Self {
        let (host, rest) = split_url(&entry.final_url);
        let info = inventory
            .targets
            .iter()
            .find(|info| info.target == entry.target);
        let is_picker = entry.target.get("picker").is_some();
        let mut badges = Vec::new();
        if entry.expanded {
            badges.push("expanded");
        }
        if entry.cleaned {
            badges.push("cleaned");
        }
        Self {
            id: entry.id,
            time: entry.time,
            host,
            rest,
            final_url: entry.final_url.clone(),
            original_url: entry.original_url.clone(),
            changed: !entry.original_url.is_empty() && entry.original_url != entry.final_url,
            source: entry.source.clone(),
            source_name: entry.source_name.clone(),
            target_name: entry.target_name.clone(),
            // The service names an installed app's icon itself when the
            // inventory does not list the app (DLG-HIS-02).
            icon: match info
                .and_then(|info| info.icon.as_deref())
                .or(entry.target_icon.as_deref())
            {
                Some(name) => icon::source(Some(name)),
                None if is_picker => icon::PICKER_ICON.to_owned(),
                None => FALLBACK_ICON.to_owned(),
            },
            same_target: !is_picker,
            reason: entry.reason.clone(),
            reason_kind: ReasonKind::of(&entry.reason),
            badges,
        }
    }
}

/// The host of `link`, and everything after it. A link without a host
/// (`mailto:`) is all host.
#[must_use]
pub fn split_url(link: &str) -> (String, String) {
    match Url::parse(link) {
        Ok(url) => match url.host_str() {
            Some(host) => {
                let port = url
                    .port()
                    .map_or_else(String::new, |port| format!(":{port}"));
                let rest = &url[Position::BeforePath..];
                (
                    format!("{host}{port}"),
                    if rest == "/" { "" } else { rest }.to_owned(),
                )
            }
            None => (link.to_owned(), String::new()),
        },
        Err(_) => (link.to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests;
