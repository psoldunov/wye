use super::*;
use crate::config::Config;
use crate::hooks::Hooks;
use crate::matcher::{MatcherKind, UrlMatcher};
use crate::pipeline::{Chosen, OpenOptions, Pipeline};
use crate::rule::{Rule, RunPosition};
use crate::source::SourceApp;
use crate::target::{Availability, DesktopId};

struct AllAvailable;

impl Availability for AllAvailable {
    fn is_available(&self, _: &Target) -> bool {
        true
    }
}

fn app(id: &str) -> Target {
    Target::App(DesktopId::new(id).unwrap())
}

fn entry(url: &str) -> HistoryEntry {
    HistoryEntry {
        id: 0,
        time: 1_700_000_000,
        original: url.to_owned(),
        url: url.to_owned(),
        entry: EntryPoint::Handler,
        source: None,
        target: app("firefox.desktop"),
        reason: Reason::Fallback,
        expanded: false,
        cleaned: false,
        transformed: false,
    }
}

fn rule(name: &str, domain: &str, target: Target) -> Rule {
    Rule {
        id: None,
        name: name.to_owned(),
        enabled: true,
        target,
        url_matchers: vec![UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: domain.to_owned(),
        }],
        source_apps: Vec::new(),
        held_keys: crate::keys::Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform: false,
    }
}

fn record(
    link: &str,
    config: Config,
    request: &LinkRequest,
    chosen: Option<Chosen>,
) -> HistoryEntry {
    let pipeline = Pipeline::with_shipped_data(config);
    let resolution = pipeline.resolve(request, &AllAvailable).unwrap();
    let finished = pipeline.finish(&resolution, request, chosen, Hooks::none());
    assert_eq!(request.url, link);
    HistoryEntry::from_link(42, request, &resolution, &finished)
}

fn config() -> Config {
    let mut config = Config::default();
    config.browsers.primary = app("firefox.desktop");
    config.browsers.alternative = app("chromium.desktop");
    config
}

// PIPE-16
#[test]
fn recording_assigns_ids_and_keeps_newest_first() {
    let history = History::new()
        .record(entry("https://a.example/"))
        .record(entry("https://b.example/"))
        .record(entry("https://c.example/"));
    let urls: Vec<_> = history.entries().iter().map(|e| e.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://c.example/",
            "https://b.example/",
            "https://a.example/"
        ]
    );
    let ids: Vec<_> = history.entries().iter().map(|e| e.id).collect();
    assert_eq!(ids, [3, 2, 1]);
    assert_eq!(history.len(), 3);
    assert!(!history.is_empty());
    assert_eq!(history.get(2).unwrap().url, "https://b.example/");
    assert!(history.get(9).is_none());
}

// ADV-09
#[test]
fn the_ring_keeps_the_newest_hundred() {
    let mut history = History::new();
    for i in 0..(CAPACITY + 25) {
        history = history.record(entry(&format!("https://example.com/{i}")));
    }
    assert_eq!(history.len(), CAPACITY);
    assert_eq!(
        history.entries()[0].url,
        format!("https://example.com/{}", CAPACITY + 24)
    );
    assert_eq!(
        history.entries()[CAPACITY - 1].url,
        "https://example.com/25"
    );
    // IDs keep counting up; none repeats.
    let mut ids: Vec<_> = history.entries().iter().map(|e| e.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), CAPACITY);
}

#[test]
fn changes_return_a_new_history() {
    let one = History::new().record(entry("https://a.example/"));
    let two = one.record(entry("https://b.example/"));
    assert_eq!(one.len(), 1, "the original is untouched");
    assert_eq!(two.len(), 2);
    let removed = two.remove(1);
    assert_eq!(two.len(), 2);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed.entries()[0].url, "https://b.example/");
    assert!(two.cleared().is_empty());
    assert_eq!(two.len(), 2);
    assert_eq!(
        two.remove(99),
        two,
        "removing an unknown entry changes nothing"
    );
}

#[test]
fn an_id_is_not_reused_after_the_newest_entry_is_deleted_and_another_recorded() {
    let history = History::new()
        .record(entry("https://a.example/"))
        .record(entry("https://b.example/"));
    let trimmed = history.remove(2).record(entry("https://c.example/"));
    assert_eq!(
        trimmed.entries()[0].id,
        2,
        "the next free id after the highest remaining"
    );
    assert_eq!(trimmed.entries()[1].id, 1);
}

// TRAY-15
#[test]
fn recent_takes_the_newest_entries() {
    let history = (0..5).fold(History::new(), |h, i| {
        h.record(entry(&format!("https://e.example/{i}")))
    });
    let recent = history.recent(3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].url, "https://e.example/4");
    assert_eq!(history.recent(50).len(), 5);
    assert!(History::new().recent(10).is_empty());
}

// DLG-HIS-01
#[test]
fn search_needs_every_word_in_any_field() {
    let mut slack = entry("https://github.com/example/repo/pull/42");
    slack.source = Some("com.slack.Slack.desktop".into());
    slack.reason = Reason::Rule {
        name: "GitHub in Firefox".into(),
    };
    let mut meet = entry("https://meet.google.com/abc-defg-hij");
    meet.source = Some("org.mozilla.Thunderbird.desktop".into());
    meet.target = Target::Profile {
        app: DesktopId::new("google-chrome.desktop").unwrap(),
        id: "Profile 1".into(),
    };
    meet.original = "https://bit.ly/meet".into();
    let history = History::new().record(slack).record(meet);

    let urls = |query: &str| -> Vec<String> {
        history
            .search(query)
            .into_iter()
            .map(|e| e.url.clone())
            .collect()
    };
    assert_eq!(urls("").len(), 2, "an empty query matches everything");
    assert_eq!(urls("   ").len(), 2);
    assert_eq!(urls("github"), ["https://github.com/example/repo/pull/42"]);
    assert_eq!(
        urls("GITHUB pull"),
        ["https://github.com/example/repo/pull/42"]
    );
    assert_eq!(urls("slack"), ["https://github.com/example/repo/pull/42"]);
    assert_eq!(
        urls("thunderbird"),
        ["https://meet.google.com/abc-defg-hij"]
    );
    assert_eq!(
        urls("bit.ly"),
        ["https://meet.google.com/abc-defg-hij"],
        "the original link"
    );
    assert_eq!(
        urls("google-chrome"),
        ["https://meet.google.com/abc-defg-hij"],
        "the target"
    );
    assert_eq!(
        urls("rule “github"),
        ["https://github.com/example/repo/pull/42"],
        "the reason"
    );
    assert!(urls("github thunderbird").is_empty());
    assert!(urls("nothing like it").is_empty());
}

// DLG-HIS-02
#[test]
fn reasons_have_labels() {
    assert_eq!(
        Reason::Rule {
            name: "Meetings".into()
        }
        .label(),
        "rule “Meetings”"
    );
    assert_eq!(
        Reason::Mapping {
            name: "Google Meet".into()
        }
        .label(),
        "web app “Google Meet”"
    );
    assert_eq!(Reason::Fallback.label(), "primary browser");
    assert_eq!(Reason::Picker.label(), "picker choice");
    assert_eq!(Reason::AlternativeKey.label(), "alternative browser key");
}

#[test]
fn badges_name_what_changed_the_link() {
    let mut e = entry("https://example.com/");
    assert!(e.badges().is_empty());
    e.cleaned = true;
    assert_eq!(e.badges(), ["cleaned"]);
    e.expanded = true;
    e.transformed = true;
    assert_eq!(e.badges(), ["expanded", "cleaned", "transformed"]);
}

// PIPE-16: built from what the pipeline decided.
#[test]
fn an_entry_from_a_fallback_link_records_the_cleaning() {
    let mut request = LinkRequest::new("https://example.com/a?utm_source=x", EntryPoint::Clipboard);
    request.source = SourceApp {
        desktop_id: Some(DesktopId::new("com.slack.Slack").unwrap()),
        executable: None,
    };
    let e = record(
        "https://example.com/a?utm_source=x",
        config(),
        &request,
        None,
    );
    assert_eq!(e.time, 42);
    assert_eq!(e.original, "https://example.com/a?utm_source=x");
    assert_eq!(e.url, "https://example.com/a");
    assert_eq!(e.entry, EntryPoint::Clipboard);
    assert_eq!(e.source.as_deref(), Some("com.slack.Slack.desktop"));
    assert_eq!(e.target, app("firefox.desktop"));
    assert_eq!(e.reason, Reason::Fallback);
    assert!(e.cleaned && !e.expanded && !e.transformed);
}

#[test]
fn an_entry_from_a_rule_or_the_alternative_key() {
    let mut with_rule = config();
    with_rule.rules = vec![rule("Meetings", "meet.google.com", app("chromium.desktop"))];
    let e = record(
        "https://meet.google.com/abc",
        with_rule,
        &LinkRequest::new("https://meet.google.com/abc", EntryPoint::Handler),
        None,
    );
    assert_eq!(
        e.reason,
        Reason::Rule {
            name: "Meetings".into()
        }
    );
    assert_eq!(e.target, app("chromium.desktop"));
    assert!(e.source.is_none(), "an unknown source is not recorded");

    let mut request = LinkRequest::new("https://example.com/", EntryPoint::Cli);
    request.held = crate::keys::Modifiers::from_slice(&[crate::keys::Modifier::Shift]);
    let e = record("https://example.com/", config(), &request, None);
    assert_eq!(e.reason, Reason::AlternativeKey);
}

#[test]
fn an_entry_from_a_mapping_uses_the_service_name() {
    let mut config = config();
    config.apps.insert("spotify".into(), app("spotify.desktop"));
    let e = record(
        "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        config,
        &LinkRequest::new(
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            EntryPoint::Handler,
        ),
        None,
    );
    assert_eq!(
        e.reason,
        Reason::Mapping {
            name: "Spotify".into()
        }
    );
}

#[test]
fn a_picker_choice_is_the_reason_whatever_sent_the_link_there() {
    let mut config = config();
    config.browsers.primary = Target::Picker;
    let e = record(
        "https://example.com/",
        config,
        &LinkRequest::new("https://example.com/", EntryPoint::Handler),
        Some(Chosen {
            target: app("chromium.desktop"),
            options: OpenOptions::default(),
        }),
    );
    assert_eq!(e.reason, Reason::Picker);
    assert_eq!(e.target, app("chromium.desktop"));
}

// DLG-HIS-02: "expanded" for wrapper and short-link expansion alike.
#[test]
fn unwrapping_marks_the_entry_as_expanded() {
    let e = record(
        "https://www.google.com/url?q=https%3A%2F%2Fexample.com%2F",
        config(),
        &LinkRequest::new(
            "https://www.google.com/url?q=https%3A%2F%2Fexample.com%2F",
            EntryPoint::Handler,
        ),
        None,
    );
    assert!(e.expanded);
    assert_eq!(e.url, "https://example.com/");
}

// 12-data-model.md, "Storage"
#[test]
fn json_round_trips_and_is_newest_first() {
    let mut first = entry("https://a.example/");
    first.source = Some("slack".into());
    first.reason = Reason::Rule { name: "R".into() };
    first.cleaned = true;
    let history = History::new()
        .record(first)
        .record(entry("https://b.example/"));
    let json = history.to_json().unwrap();
    assert!(json.contains("\"version\": 1"));
    assert_eq!(History::from_json(&json).unwrap(), history);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["entries"][0]["url"], "https://b.example/");
    assert_eq!(
        value["entries"][1]["reason"],
        serde_json::json!({ "kind": "rule", "name": "R" })
    );
    assert_eq!(value["entries"][1]["entry"], "handler");
    assert_eq!(
        value["entries"][0]["target"],
        serde_json::json!({ "app": "firefox.desktop" })
    );
}

#[test]
fn reading_keeps_at_most_the_capacity_and_reports_bad_files() {
    let mut history = History::new();
    for i in 0..CAPACITY {
        history = history.record(entry(&format!("https://e.example/{i}")));
    }
    let mut value: serde_json::Value = serde_json::from_str(&history.to_json().unwrap()).unwrap();
    let extra = value["entries"][0].clone();
    for _ in 0..10 {
        value["entries"].as_array_mut().unwrap().push(extra.clone());
    }
    let read = History::from_json(&value.to_string()).unwrap();
    assert_eq!(read.len(), CAPACITY);

    assert!(matches!(
        History::from_json("not json"),
        Err(HistoryError::Invalid(_))
    ));
    assert!(matches!(
        History::from_json("{}"),
        Err(HistoryError::Invalid(_))
    ));
    let future = r#"{"version":2,"entries":[]}"#;
    assert!(matches!(
        History::from_json(future),
        Err(HistoryError::Version { found: 2 })
    ));
    assert!(
        History::from_json(r#"{"version":1,"entries":[]}"#)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_missing_optional_field_reads_as_its_default() {
    let json = r#"{"version":1,"entries":[{"id":7,"time":5,"original":"https://a/","url":"https://a/","entry":"cli","target":{"picker":true},"reason":{"kind":"fallback"}}]}"#;
    let history = History::from_json(json).unwrap();
    let e = &history.entries()[0];
    assert_eq!(e.source, None);
    assert!(!e.cleaned && !e.expanded && !e.transformed);
    assert_eq!(e.entry, EntryPoint::Cli);
}
