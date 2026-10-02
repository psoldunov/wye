//! History and rules on a private bus (DLG-HIS-01 to DLG-HIS-03, ADV-09,
//! TRAY-15, IN-08, RUL-02). Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;

use support::{ONE, Service, eventually};
use wye_api::trace::{LinkTrace, TraceDecision};
use wye_api::{Error, context, json};
use wye_core::Target;
use wye_core::history::{History, HistoryEntry, Reason};
use wye_core::pipeline::EntryPoint;
use zbus::zvariant::Value;

const HISTORY: &str = "state/wye/history.json";

fn entry(url: &str, target: Target) -> HistoryEntry {
    HistoryEntry {
        id: 0,
        time: 1_700_000_000,
        original: url.to_owned(),
        url: url.to_owned(),
        entry: EntryPoint::Handler,
        source: None,
        target,
        reason: Reason::Fallback,
        expanded: false,
        cleaned: true,
        transformed: false,
    }
}

/// A service whose history file holds two links, newest first: `/two`
/// (ID 2) and `/one` (ID 1), both opened in Fake One. The file is written
/// before the service starts: the service reads it once (decision 10), and
/// a startup task such as the tray's "Recent Links" may do so at once.
async fn with_history(config: &str) -> Option<Service> {
    let one = Target::App(wye_core::DesktopId::new(ONE).expect("valid"));
    let history = History::new()
        .record(entry("https://example.com/one", one.clone()))
        .record(entry("https://example.com/two", one));
    let text = history.to_json().expect("JSON");
    Service::start_with(config, |desktop| desktop.write(HISTORY, &text)).await
}

async fn history(service: &Service) -> wye_api::history::History {
    let text = service.wye().await.get_history().await.expect("GetHistory");
    json::decode("history", &text).expect("History")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dlg_his_the_stored_links_are_listed_with_names() {
    let Some(service) = with_history("[advanced]\nhistory = true\n").await else {
        return;
    };
    let listed = history(&service).await;
    assert!(listed.enabled, "ADV-09");
    let urls: Vec<&str> = listed
        .entries
        .iter()
        .map(|entry| entry.final_url.as_str())
        .collect();
    assert_eq!(urls, ["https://example.com/two", "https://example.com/one"]);
    assert_eq!(listed.entries[0].target_name, "Fake One");
    assert_eq!(listed.entries[0].reason, "primary browser");
    assert!(listed.entries[0].cleaned);
}

/// DLG-HIS-02: an entry whose target is an installed app the configuration
/// no longer names (a custom app, or an app no service lists) still shows
/// the app's name and icon, not its desktop file name.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dlg_his02_installed_apps_the_config_does_not_name_keep_name_and_icon() {
    let figma = wye_core::DesktopId::new("figma-linux-next.desktop").expect("valid");
    let stored = History::new()
        .record(entry(
            "https://www.figma.com/file/one",
            Target::Custom(wye_core::CustomApp::Desktop(figma.clone())),
        ))
        .record(entry("https://www.figma.com/file/two", Target::App(figma)));
    let text = stored.to_json().expect("JSON");
    let Some(service) = Service::start_with("[advanced]\nhistory = true\n", |desktop| {
        desktop.write(
            "data/applications/figma-linux-next.desktop",
            "[Desktop Entry]\nType=Application\nName=Figma Linux Next\n\
             Icon=figma-linux\nExec=figma-linux-next %u\n",
        );
        desktop.write(HISTORY, &text);
    })
    .await
    else {
        return;
    };
    let listed = history(&service).await;
    let shown: Vec<(&str, Option<&str>)> = listed
        .entries
        .iter()
        .map(|entry| (entry.target_name.as_str(), entry.target_icon.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [
            ("Figma Linux Next", Some("figma-linux")),
            ("Figma Linux Next", Some("figma-linux")),
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dlg_his03_delete_and_clear_persist_and_bump_the_revision() {
    let Some(service) = with_history("").await else {
        return;
    };
    let wye = service.wye().await;
    let before = wye.history_revision().await.expect("revision");
    wye.delete_history_entry(2).await.expect("deleted");
    // The proxy caches the property until PropertiesChanged arrives.
    eventually("HistoryRevision bumps", || async {
        wye.history_revision().await.is_ok_and(|now| now > before)
    })
    .await;
    assert_eq!(history(&service).await.entries.len(), 1);
    let gone = wye.delete_history_entry(2).await;
    assert!(matches!(gone, Err(Error::NotFound(_))), "{gone:?}");

    wye.clear_history().await.expect("cleared");
    assert!(history(&service).await.entries.is_empty());
    let stored = History::from_json(&service.desktop.read(HISTORY)).expect("history file");
    assert!(stored.is_empty(), "DLG-HIS-01: cleared on disk");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tray15_reopen_in_the_same_target_launches_it() {
    let Some(service) = with_history("").await else {
        return;
    };
    let wye = service.wye().await;
    wye.reopen_history_entry(1, "same-target")
        .await
        .expect("reopened");
    assert_eq!(
        service.launched(),
        [vec![
            "fake-one".to_owned(),
            "https://example.com/one".to_owned()
        ]]
    );
    let unknown_mode = wye.reopen_history_entry(1, "sideways").await;
    assert!(
        matches!(unknown_mode, Err(Error::InvalidArgs(_))),
        "{unknown_mode:?}"
    );
    let unknown_id = wye.reopen_history_entry(9, "picker").await;
    assert!(
        matches!(unknown_id, Err(Error::NotFound(_))),
        "{unknown_id:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in08_test_link_traces_without_opening() {
    let config = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";
    let Some(service) = Service::start(config).await else {
        return;
    };
    let link = HashMap::from([(context::ENTRY, Value::from("cli"))]);
    let text = service
        .wye()
        .await
        .test_link("https://example.com/?utm_source=x", link)
        .await
        .expect("traced");
    let trace: LinkTrace = json::decode("trace", &text).expect("LinkTrace");
    assert_eq!(trace.decision, TraceDecision::Open);
    assert_eq!(trace.target_name.as_deref(), Some("Fake One"));
    assert_eq!(trace.final_url.as_deref(), Some("https://example.com/"));
    assert!(trace.steps.iter().any(|step| step.kind == "clean"));
    assert!(service.launched().is_empty(), "nothing is opened");

    let rejected: LinkTrace = json::decode(
        "trace",
        &service
            .wye()
            .await
            .test_link("ftp://example.com/", HashMap::new())
            .await
            .expect("traced"),
    )
    .expect("LinkTrace");
    assert_eq!(rejected.decision, TraceDecision::Rejected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rul02_rules_round_trip_through_export_and_import() {
    let config = "[[rules]]\nname = \"GitHub\"\ntarget = { app = \"fake-one.desktop\" }\n\
                  url-matchers = [{ kind = \"domain\", pattern = \"github.com\" }]\n";
    let Some(service) = Service::start(config).await else {
        return;
    };
    let wye = service.wye().await;
    let exported = wye.export_rules().await.expect("exported");
    assert!(exported.contains("GitHub"), "{exported}");
    assert_eq!(wye.import_rules(&exported).await.expect("imported"), 1);
    let (text, _) = wye.get_config().await.expect("GetConfig");
    let config: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(config["rules"].as_array().map(Vec::len), Some(2));

    let invalid = wye.import_rules("not a rules file").await;
    assert!(matches!(invalid, Err(Error::InvalidArgs(_))), "{invalid:?}");
}
