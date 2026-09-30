//! The `KWin` helper against [`FakeScriptHost`] (PICK-02, source-app step 4).

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::platform::Platform;
use crate::platform::fake::FakeScriptHost;

const KWIN: &str = ":1.9";

/// Short enough to keep the silent-compositor tests quick.
const QUICK: Duration = Duration::from_millis(40);

fn answer() -> KWinReport {
    KWinReport {
        nonce: String::new(),
        pointer: Placement {
            output: "DP-1".to_owned(),
            x: 451,
            y: 306,
        },
        pid: 4242,
        desktop_file: "org.kde.dolphin".to_owned(),
        resource_class: "dolphin".to_owned(),
    }
}

fn reply_to() -> ReplyTo {
    ReplyTo {
        service: ":1.100".to_owned(),
        path: OBJECT_PATH.to_owned(),
        interface: KWIN_INTERFACE.to_owned(),
    }
}

fn helper(host: &Arc<FakeScriptHost>, reports: &Reports, dir: &Path) -> KWinHelper {
    KWinHelper::new(
        Arc::clone(host) as _,
        reports.clone(),
        reply_to(),
        dir.to_path_buf(),
    )
    .with_timeout(QUICK)
}

#[tokio::test]
async fn an_answered_query_gives_pointer_and_focus_then_cleans_up() {
    let dir = tempfile::tempdir().expect("temp dir");
    let reports = Reports::new();
    let host = Arc::new(FakeScriptHost::answering(KWIN, reports.clone(), answer()));
    let helper = helper(&host, &reports, dir.path());

    assert_eq!(helper.pointer().await, Some(answer().pointer));
    assert_eq!(
        helper.focused().await,
        Some(FocusedApp {
            pid: Some(4242),
            desktop_id: Some("org.kde.dolphin".to_owned()),
            resource_class: Some("dolphin".to_owned()),
        })
    );

    let calls = host.calls();
    assert_eq!(calls.loaded.len(), 1, "pointer and focus share one query");
    let (name, text) = &calls.loaded[0];
    assert!(name.starts_with(script::NAME_PREFIX));
    assert!(text.contains(":1.100"), "answers to our connection");
    assert_eq!(calls.unloaded, vec![name.clone()]);
    assert_eq!(reports.waiting(), 0);
    let left = std::fs::read_dir(dir.path()).expect("dir").count();
    assert_eq!(left, 0, "the script file is removed");
}

/// On a paused clock, so the deadline is exact however busy the machine
/// is: the query gives up at [`QUICK`], and cleaning up takes no time.
#[tokio::test(start_paused = true)]
async fn a_silent_compositor_times_out_to_unknown_and_still_unloads() {
    let dir = tempfile::tempdir().expect("temp dir");
    let reports = Reports::new();
    let host = Arc::new(FakeScriptHost::silent(KWIN));
    let helper = helper(&host, &reports, dir.path());

    let started = tokio::time::Instant::now();
    assert_eq!(helper.pointer().await, None);
    assert_eq!(started.elapsed(), QUICK);

    let calls = host.calls();
    assert_eq!(calls.started, 1);
    assert_eq!(calls.unloaded.len(), 1);
    assert_eq!(reports.waiting(), 0);
}

#[tokio::test]
async fn without_a_compositor_nothing_is_loaded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let reports = Reports::new();
    let host = Arc::new(FakeScriptHost::default());
    let helper = helper(&host, &reports, dir.path());

    assert_eq!(helper.focused().await, None);
    assert!(host.calls().loaded.is_empty());
}

#[test]
fn only_the_compositor_may_answer() {
    let reports = Reports::new();
    let mut answer_rx = reports.expect("abc", KWIN);
    let report = KWinReport {
        nonce: "abc".to_owned(),
        ..answer()
    };

    let refused = reports.deliver(Some(":1.666"), report.clone());
    assert!(matches!(refused, Err(Error::InvalidArgs(_))), "{refused:?}");
    assert!(reports.deliver(None, report.clone()).is_err());
    assert_eq!(reports.waiting(), 1, "the query keeps waiting");

    assert!(matches!(
        reports.deliver(Some(KWIN), report.clone()),
        Ok(Delivery::Delivered)
    ));
    assert_eq!(answer_rx.try_recv(), Ok(report));
    assert_eq!(reports.waiting(), 0);
}

#[test]
fn a_report_nobody_waits_for_is_dropped() {
    let reports = Reports::new();
    let late = KWinReport {
        nonce: "gone".to_owned(),
        ..answer()
    };
    assert!(matches!(
        reports.deliver(Some(KWIN), late),
        Ok(Delivery::Unexpected)
    ));

    let receiver = reports.expect("timed-out", KWIN);
    drop(receiver);
    let dropped = KWinReport {
        nonce: "timed-out".to_owned(),
        ..answer()
    };
    assert!(matches!(
        reports.deliver(Some(KWIN), dropped),
        Ok(Delivery::Unexpected)
    ));
}

#[test]
fn the_bus_handler_delivers_through_the_platform() {
    let platform = Platform::unavailable();
    let reports = platform.kwin_reports.clone();
    let ctx = ServiceContext::new(platform);
    let mut waiting = reports.expect("n1", KWIN);
    let caller = Caller {
        sender: Some(KWIN.to_owned()),
    };
    let report = KWinReport {
        nonce: "n1".to_owned(),
        ..answer()
    };

    super::report(&ctx, &caller, report.clone()).expect("accepted");
    assert_eq!(waiting.try_recv(), Ok(report));

    let stranger = Caller {
        sender: Some(":1.7".to_owned()),
    };
    let _still = reports.expect("n2", KWIN);
    let spoofed = KWinReport {
        nonce: "n2".to_owned(),
        ..answer()
    };
    assert!(super::report(&ctx, &stranger, spoofed).is_err());
    let unknown = KWinReport {
        nonce: "n3".to_owned(),
        ..answer()
    };
    assert!(super::report(&ctx, &caller, unknown).is_ok());
}

#[test]
fn empty_fields_mean_unknown() {
    let nothing = KWinReport::default();
    assert_eq!(nothing.placement(), None);
    assert_eq!(nothing.focused(), None);

    let class_only = KWinReport {
        resource_class: "foot".to_owned(),
        ..KWinReport::default()
    };
    assert_eq!(
        class_only.focused(),
        Some(FocusedApp {
            resource_class: Some("foot".to_owned()),
            ..FocusedApp::default()
        })
    );
}

#[test]
fn nonces_are_long_hex_and_differ() {
    let first = nonce();
    let second = nonce();
    assert_eq!(first.len(), 32);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(first, second);
}
