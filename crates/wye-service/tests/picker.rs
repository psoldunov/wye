//! The picker round trip on a private bus (PIPE-13, PICK-23, PICK-27,
//! PICK-29, PICK-31, PICK-33, PKS-06, PKS-07, IN-06): a fake UI host owns
//! `dev.soldunov.wye.Ui` and records what the service asks of it; the test
//! answers the way the picker would. The frontends (ADV-12) are in
//! `frontends.rs`. Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;

use support::hosts::{Call, PICKER, URL, cli, fake_ui, shown_count, two, wye};
use support::{Service, eventually};
use wye_api::{Error, context};
use zbus::zvariant::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_choice_opens_the_link_with_the_pickers_token() {
    // PIPE-13, PICK-29, PICK-33.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let (id, request) = ui.shown().remove(0);
    assert_eq!(request.url.full, URL);
    assert!(!request.preview);
    assert!(
        service.launched().is_empty(),
        "nothing opens before the choice"
    );

    let options = HashMap::from([
        (context::OPTION_NEW_WINDOW, Value::from(true)),
        (
            context::OPTION_ACTIVATION_TOKEN,
            Value::from("picker-token"),
        ),
    ]);
    proxy
        .picker_chose(&id, &two(), options)
        .await
        .expect("chosen");
    let launched = service.fakes.launcher.launched();
    assert_eq!(launched.len(), 1);
    assert_eq!(
        launched[0].argv.first().map(String::as_str),
        Some("fake-two")
    );
    let token = launched[0]
        .env
        .iter()
        .find(|(key, _)| key == "XDG_ACTIVATION_TOKEN")
        .and_then(|(_, value)| value.clone());
    assert_eq!(token.as_deref(), Some("picker-token"));

    let again = proxy.picker_chose(&id, &two(), HashMap::new()).await;
    assert!(matches!(again, Err(Error::NotFound(_))), "{again:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_link_supersedes_the_pending_one() {
    // PICK-27: the old request's answer finds nothing; the new link opens.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy
        .open_link("https://example.com/old", cli())
        .await
        .expect("routed");
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 2).await;
    let shown = ui.shown();
    let (old, new) = (&shown[0].0, &shown[1].0);
    assert_ne!(old, new);
    assert_eq!(shown[1].1.url.full, URL);

    let stale = proxy.picker_chose(old, &two(), HashMap::new()).await;
    assert!(matches!(stale, Err(Error::NotFound(_))), "{stale:?}");
    proxy
        .picker_chose(new, &two(), HashMap::new())
        .await
        .expect("chosen");
    assert_eq!(
        service.launched(),
        [vec!["fake-two".to_owned(), URL.to_owned()]]
    );
    // One host shows both: its new request replaced the old one, so it is
    // never asked to close it (as before ADV-12).
    assert!(
        !ui.calls()
            .iter()
            .any(|call| matches!(call, Call::Close { .. })),
        "{:?}",
        ui.calls()
    );
}

/// KEY-13: the picker's private-window choice of a shown browser is its
/// private target, which the service accepts and opens privately.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_private_window_choice_opens_privately() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    service
        .desktop
        .app("firefox.desktop", "Firefox", "firefox %u", true);
    let proxy = wye(&service).await;
    proxy.rescan().await.expect("rescanned");
    let (ui, _ui_connection) = fake_ui(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let id = ui.shown().remove(0).0;
    proxy
        .picker_chose(&id, r#"{"private":"firefox.desktop"}"#, HashMap::new())
        .await
        .expect("a private choice is accepted");
    let launched = service.fakes.launcher.launched();
    assert_eq!(launched.len(), 1);
    assert_eq!(
        launched[0].argv.first().map(String::as_str),
        Some("firefox")
    );
    assert!(
        launched[0].argv.iter().any(|arg| arg == "--private-window"),
        "{:?}",
        launched[0].argv
    );
}

/// PIPE-13: only a target the request showed is accepted; anything else
/// is refused and the request stays pending.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_target_the_picker_did_not_show_is_refused() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let id = ui.shown().remove(0).0;
    let refused = proxy
        .picker_chose(&id, r#"{"custom":"/bin/sh"}"#, HashMap::new())
        .await;
    assert!(matches!(refused, Err(Error::InvalidArgs(_))), "{refused:?}");
    assert!(service.launched().is_empty());
    proxy
        .picker_chose(&id, &two(), HashMap::new())
        .await
        .expect("still pending");
    assert_eq!(service.fakes.launcher.launched().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_links_activation_token_reaches_the_picker() {
    // PICK-01, LAUNCH-03: a picker window may take the focus with it.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let mut context = cli();
    context.insert(context::ACTIVATION_TOKEN, Value::from("link-token"));
    wye(&service)
        .await
        .open_link(URL, context)
        .await
        .expect("routed");
    shown_count(&ui, 1).await;
    let (_, request) = ui.shown().remove(0);
    assert_eq!(request.activation_token.as_deref(), Some("link-token"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_opens_nothing() {
    // PICK-23.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let id = ui.shown().remove(0).0;
    proxy.picker_cancelled(&id).await.expect("cancelled");
    assert!(service.launched().is_empty());
    let again = proxy.picker_cancelled(&id).await;
    assert!(matches!(again, Err(Error::NotFound(_))), "{again:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_preview_opens_nothing() {
    // IN-06, PKS-06.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.preview_picker().await.expect("previewed");
    let (id, request) = ui.shown().remove(0);
    assert!(request.preview);
    proxy
        .picker_chose(&id, &two(), HashMap::new())
        .await
        .expect("chosen");
    assert!(service.launched().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_ui_the_stand_in_opens_the_link_and_says_so() {
    // PIPE-13: the link never goes nowhere.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    wye(&service)
        .await
        .open_link(URL, cli())
        .await
        .expect("decided");
    // `OpenLink` answers once the link is decided; the stand-in follows.
    eventually("the stand-in to open", || async {
        service.launched().len() == 1
    })
    .await;
    let shown = service.fakes.notifier.shown();
    assert!(
        shown
            .iter()
            .any(|notification| notification.summary.contains("picker")),
        "{shown:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn locking_the_screen_closes_the_picker_and_keeps_the_link() {
    // PKS-07: the link waits for the unlock and the picker comes back.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let first = ui.shown().remove(0).0;

    service.fakes.lock.set(true);
    eventually("the picker to close", || async {
        ui.calls().contains(&Call::Close { id: first.clone() })
    })
    .await;
    service.fakes.lock.set(false);
    shown_count(&ui, 2).await;
    let (second, request) = ui.shown().remove(1);
    assert_ne!(first, second);
    assert_eq!(request.url.full, URL);
    assert!(service.launched().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn picker_actions_copy_the_link_or_open_the_rule_editor() {
    // KEY-22 copy-link; PICK-31 create-rule. Neither opens the link.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let (ui, _ui_connection) = fake_ui(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    let id = ui.shown().remove(0).0;
    proxy.picker_action(&id, "copy-link").await.expect("copied");
    assert_eq!(service.fakes.clipboard.written(), [URL]);

    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 2).await;
    let id = ui.shown().remove(1).0;
    proxy.picker_action(&id, "create-rule").await.expect("rule");
    let windows: Vec<_> = ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            Call::Window { window, argument } => Some((window, argument)),
            _ => None,
        })
        .collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].0, "rule-editor");
    let prefill: serde_json::Value = serde_json::from_str(&windows[0].1).expect("json");
    assert_eq!(prefill["domain"], "example.com");
    assert!(service.launched().is_empty());
    let bad = proxy.picker_action(&id, "dance").await;
    assert!(matches!(bad, Err(Error::InvalidArgs(_))), "{bad:?}");
}
