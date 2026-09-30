//! The history window's search (DLG-HIS-01): the same rule as
//! `wye_core::history::History::search`, on the entries the service sent.

use wye_api::history::HistoryEntry;

/// The entries that match `query`, in their order. The query is split at
/// white space; every word must appear, in any case, in the link, the
/// original link, the source app, the target or the reason. An empty query
/// matches everything.
#[must_use]
pub fn matching<'a>(entries: &'a [HistoryEntry], query: &str) -> Vec<&'a HistoryEntry> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    entries
        .iter()
        .filter(|entry| matches(entry, &terms))
        .collect()
}

fn matches(entry: &HistoryEntry, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let haystack = [
        entry.final_url.as_str(),
        &entry.original_url,
        entry.source.as_deref().unwrap_or_default(),
        entry.source_name.as_deref().unwrap_or_default(),
        &entry.target_name,
        &entry.reason,
    ]
    .join("\n")
    .to_lowercase();
    terms.iter().all(|term| haystack.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, url: &str, source: &str, target: &str, reason: &str) -> HistoryEntry {
        HistoryEntry {
            id,
            final_url: url.to_owned(),
            original_url: url.to_owned(),
            source_name: Some(source.to_owned()),
            target_name: target.to_owned(),
            reason: reason.to_owned(),
            ..HistoryEntry::default()
        }
    }

    fn sample() -> Vec<HistoryEntry> {
        vec![
            entry(
                2,
                "https://meet.google.com/abc",
                "Thunderbird",
                "Work (Chrome)",
                "rule “Meetings”",
            ),
            entry(
                1,
                "https://github.com/example/repo",
                "Slack",
                "Firefox",
                "picker choice",
            ),
        ]
    }

    #[test]
    fn an_empty_query_keeps_everything_in_order() {
        // DLG-HIS-01
        let entries = sample();
        let ids: Vec<u64> = matching(&entries, "  ").iter().map(|e| e.id).collect();
        assert_eq!(ids, [2, 1]);
    }

    #[test]
    fn every_word_must_match_in_any_field_and_case() {
        let entries = sample();
        let ids = |query| -> Vec<u64> { matching(&entries, query).iter().map(|e| e.id).collect() };
        assert_eq!(ids("SLACK github"), [1]);
        assert_eq!(ids("meetings chrome"), [2]);
        assert_eq!(ids("slack meet"), Vec::<u64>::new());
    }

    #[test]
    fn the_original_link_is_searched_too() {
        let mut changed = entry(
            3,
            "https://a.example/x",
            "Slack",
            "Firefox",
            "picker choice",
        );
        changed.original_url = "https://bit.ly/short".to_owned();
        let entries = [changed];
        assert_eq!(matching(&entries, "bit.ly").len(), 1);
    }
}
