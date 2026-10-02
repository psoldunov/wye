use serde_json::json;
use wye_api::targets::{TargetInfo, TargetKind};

use super::*;

fn entry(id: u64, time: i64, url: &str) -> HistoryEntry {
    HistoryEntry {
        id,
        time,
        original_url: url.to_owned(),
        final_url: url.to_owned(),
        target: json!({"app": "firefox.desktop"}),
        target_name: "Firefox".to_owned(),
        reason: "picker choice".to_owned(),
        ..HistoryEntry::default()
    }
}

fn firefox() -> TargetInfo {
    TargetInfo {
        target: json!({"app": "firefox.desktop"}),
        kind: TargetKind::App,
        name: "Firefox".to_owned(),
        short_name: None,
        icon: Some("firefox".to_owned()),
        badge: None,
        browser: None,
        capabilities: wye_api::TargetCapabilities::default(),
        packaging: None,
        missing: false,
    }
}

fn inventory() -> TargetInventory {
    TargetInventory {
        targets: vec![firefox()],
    }
}

#[test]
fn rows_are_newest_first_whatever_order_they_arrive_in() {
    // DLG-HIS-02
    let history = History {
        enabled: true,
        entries: vec![
            entry(1, 100, "https://a.example/"),
            entry(2, 300, "https://b.example/"),
            entry(3, 200, "https://c.example/"),
        ],
    };
    let view = View::build(&history, &inventory(), "");
    let ids: Vec<u64> = view.rows.iter().map(|row| row.id).collect();
    assert_eq!(ids, [2, 3, 1]);
    assert_eq!(view.total, 3);
}

#[test]
fn history_switched_off_lists_nothing() {
    // DLG-HIS-04: "History Is Off" alone, not over the entries the service
    // still keeps.
    let history = History {
        enabled: false,
        entries: vec![entry(1, 100, "https://a.example/")],
    };
    let view = View::build(&history, &inventory(), "");
    assert!(!view.enabled);
    assert!(view.rows.is_empty());
    assert_eq!(view.total, 0);
}

#[test]
fn the_search_narrows_the_rows_but_not_the_total() {
    // DLG-HIS-01
    let history = History {
        enabled: true,
        entries: vec![
            entry(1, 100, "https://a.example/"),
            entry(2, 300, "https://b.example/"),
        ],
    };
    let view = View::build(&history, &inventory(), "b.example");
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.total, 2);
}

#[test]
fn a_link_splits_into_an_emphasised_host_and_the_rest() {
    // DLG-HIS-02
    assert_eq!(
        split_url("https://github.com/example/repo/pull/42?tab=files#x"),
        (
            "github.com".to_owned(),
            "/example/repo/pull/42?tab=files#x".to_owned()
        )
    );
    assert_eq!(
        split_url("http://localhost:8080/a"),
        ("localhost:8080".to_owned(), "/a".to_owned())
    );
    assert_eq!(
        split_url("https://example.org/"),
        ("example.org".to_owned(), String::new())
    );
    assert_eq!(
        split_url("mailto:a@b.example"),
        ("mailto:a@b.example".to_owned(), String::new())
    );
    assert_eq!(
        split_url("not a link"),
        ("not a link".to_owned(), String::new())
    );
}

#[test]
fn a_known_target_brings_its_icon_and_the_picker_cannot_be_reopened_in() {
    // DLG-HIS-02, DLG-HIS-03
    let mut in_picker = entry(2, 200, "https://b.example/");
    in_picker.target = json!({"picker": true});
    in_picker.target_name = "Picker".to_owned();
    let history = History {
        enabled: true,
        entries: vec![entry(1, 100, "https://a.example/"), in_picker],
    };
    let view = View::build(&history, &inventory(), "");
    let picker = &view.rows[0];
    assert_eq!(picker.icon, icon::PICKER_ICON);
    assert!(!picker.same_target);
    let app = &view.rows[1];
    assert_eq!(app.icon, "firefox");
    assert!(app.same_target);
}

#[test]
fn an_unknown_target_gets_the_browser_icon() {
    let mut gone = entry(1, 100, "https://a.example/");
    gone.target = json!({"app": "gone.desktop"});
    let history = History {
        enabled: true,
        entries: vec![gone],
    };
    assert_eq!(
        View::build(&history, &inventory(), "").rows[0].icon,
        FALLBACK_ICON
    );
}

#[test]
fn an_app_the_inventory_does_not_list_takes_the_entrys_icon() {
    // DLG-HIS-02: an installed custom app the configuration no longer names.
    let mut custom = entry(1, 100, "https://a.example/");
    custom.target = json!({"custom": "figma-linux-next.desktop"});
    custom.target_icon = Some("/opt/figma/icon.png".to_owned());
    let history = History {
        enabled: true,
        entries: vec![custom],
    };
    // Each frontend's own form of an icon path.
    assert_eq!(
        View::build(&history, &inventory(), "").rows[0].icon,
        icon::source(Some("/opt/figma/icon.png"))
    );
}

#[test]
fn changed_links_show_their_badges_and_keep_the_original() {
    // DLG-HIS-02
    let mut changed = entry(1, 100, "https://a.example/x");
    changed.original_url = "https://bit.ly/abc".to_owned();
    changed.expanded = true;
    changed.cleaned = true;
    let history = History {
        enabled: true,
        entries: vec![changed],
    };
    let row = &View::build(&history, &inventory(), "").rows[0];
    assert!(row.changed);
    assert_eq!(row.badges, ["expanded", "cleaned"]);
    assert_eq!(row.original_url, "https://bit.ly/abc");
}

#[test]
fn the_reason_label_gives_the_badge_class() {
    assert_eq!(ReasonKind::of("rule “Meetings”"), ReasonKind::Rule);
    assert_eq!(ReasonKind::of("web app “Discord”"), ReasonKind::Mapping);
    assert_eq!(ReasonKind::of("picker choice"), ReasonKind::Picker);
    assert_eq!(ReasonKind::of("primary browser"), ReasonKind::Fallback);
    assert_eq!(
        ReasonKind::of("alternative browser key"),
        ReasonKind::Alternative
    );
    assert_eq!(ReasonKind::of("something new"), ReasonKind::Other);
}

#[test]
fn history_that_is_off_says_so() {
    // DLG-HIS-04
    let view = View::build(&History::default(), &inventory(), "");
    assert!(!view.enabled);
    assert!(view.rows.is_empty());
}
