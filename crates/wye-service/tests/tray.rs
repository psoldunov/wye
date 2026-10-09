//! The tray on a private bus (TRAY-01 to TRAY-20, decision 8): the `Tray`
//! property, `ActivateTrayItem`, an external host's `RegisterTray` hiding
//! the `StatusNotifierItem` and bringing it back, and the item's own events.
//! Skips without `dbus-daemon`.

mod support;

use serde_json::Value;
use serde_json::json;
use support::{ONE, Service, eventually};
use wye_api::Error;
use wye_api::context::Modifier;
use wye_api::proxy::Wye1Proxy;
use wye_api::tray::{TrayItem, TrayItemKind, TrayMenu};
use wye_service::platform::TrayEvent;

fn find<'a>(items: &'a [TrayItem], id: &str) -> Option<&'a TrayItem> {
    items.iter().find_map(|item| {
        if item.id == id {
            Some(item)
        } else {
            find(&item.children, id)
        }
    })
}

async fn tray(wye: &Wye1Proxy<'_>) -> TrayMenu {
    let text = wye.tray().await.expect("the Tray property");
    serde_json::from_str(&text).expect("Tray is JSON")
}

async fn primary(wye: &Wye1Proxy<'_>) -> Value {
    let (text, _) = wye.get_config().await.expect("GetConfig");
    let config: Value = serde_json::from_str(&text).expect("config JSON");
    config["browsers"]["primary"].clone()
}

/// The first shown browser's radio item.
const FIRST_BROWSER: &str = "primary:0";

/// One shown browser, Fake One.
const SHOWN: &str = "[[browsers.shown]]\ntarget = { app = \"fake-one.desktop\" }\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tray_property_is_the_menu_and_the_item_shows_it_tray_01() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    let menu = tray(&wye).await;
    assert!(menu.visible);
    let picker = find(&menu.items, "primary:picker").expect("the Picker radio");
    assert_eq!(picker.kind, TrayItemKind::Radio);
    assert!(picker.checked, "the default primary is the Picker");
    assert!(
        find(&menu.items, FIRST_BROWSER).is_some(),
        "a shown browser"
    );

    let sni = service.fakes.sni.clone();
    eventually("the StatusNotifierItem shows", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;
    assert_eq!(sni.shown(), Some(menu));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choosing_a_browser_sets_the_primary_and_redraws_the_item_tray_11() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    wye.activate_tray_item(FIRST_BROWSER)
        .await
        .expect("the radio item works");
    assert_eq!(primary(&wye).await, json!({ "app": ONE }));
    let menu = tray(&wye).await;
    assert!(find(&menu.items, FIRST_BROWSER).expect("item").checked);
    assert!(!find(&menu.items, "primary:picker").expect("item").checked);

    let sni = service.fakes.sni.clone();
    eventually("the item follows the change", || {
        let sni = sni.clone();
        async move {
            sni.shown()
                .and_then(|shown| find(&shown.items, FIRST_BROWSER).map(|item| item.checked))
                .unwrap_or(false)
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ctrl_or_shift_opens_the_browser_and_keeps_the_primary_tray_20() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    for held in [Modifier::Ctrl, Modifier::Shift] {
        service.fakes.modifiers.set(Some(vec![held]));
        wye.activate_tray_item(FIRST_BROWSER)
            .await
            .expect("the radio item opens its browser");
    }
    assert_eq!(
        service.launched(),
        [vec!["fake-one".to_owned()], vec!["fake-one".to_owned()]],
        "started without a link"
    );
    assert_eq!(
        primary(&wye).await,
        json!({ "picker": true }),
        "the primary stays"
    );
    let menu = tray(&wye).await;
    assert!(find(&menu.items, "primary:picker").expect("item").checked);

    // The Picker has nothing to open.
    wye.activate_tray_item("primary:picker")
        .await
        .expect("a modifier click on the Picker is not an error");
    assert_eq!(service.launched().len(), 2, "nothing more started");
    assert_eq!(primary(&wye).await, json!({ "picker": true }));

    // Other modifiers keep TRAY-11.
    service.fakes.modifiers.set(Some(vec![Modifier::Alt]));
    wye.activate_tray_item(FIRST_BROWSER)
        .await
        .expect("the radio item works");
    assert_eq!(primary(&wye).await, json!({ "app": ONE }));
    assert_eq!(service.launched().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_items_own_events_are_carried_out() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let sni = service.fakes.sni.clone();
    eventually("the StatusNotifierItem shows", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;
    sni.send(TrayEvent::Activated(FIRST_BROWSER.to_owned()));
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    let chosen = json!({ "app": ONE });
    for _ in 0..250 {
        if primary(&wye).await == chosen {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(primary(&wye).await, chosen, "the chosen browser is primary");

    sni.send(TrayEvent::Activated("quit".to_owned()));
    let ctx = service.ctx.clone();
    eventually("Quit Wye stops the service (TRAY-17)", || {
        let ctx = ctx.clone();
        async move { ctx.shutdown_requested() }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_registered_tray_host_hides_the_item_until_it_leaves() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let sni = service.fakes.sni.clone();
    eventually("the StatusNotifierItem shows", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;

    let panel = service.bus.connect().await;
    let host = Wye1Proxy::new(&panel).await.expect("proxy");
    host.register_tray("plasma-applet")
        .await
        .expect("an external host registers");
    eventually("the item hides for the host", || {
        let sni = sni.clone();
        async move { sni.shown().is_none() }
    })
    .await;

    drop(host);
    panel.close().await.expect("the host leaves");
    eventually("the item comes back without the host", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hiding_the_tray_icon_removes_the_item_tray_04() {
    let Some(service) = Service::start("[general]\nshow-tray-icon = false\n").await else {
        return;
    };
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    assert!(!tray(&wye).await.visible);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(service.fakes.sni.shown(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_hosts_and_items_are_refused() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let wye = Wye1Proxy::new(&service.client).await.expect("proxy");
    for id in ["separator:0", "primary-header", "more", "nothing"] {
        let refused = wye.activate_tray_item(id).await;
        assert!(
            matches!(refused, Err(Error::InvalidArgs(_))),
            "{id}: {refused:?}"
        );
    }
    let refused = wye.register_tray("xfce-panel").await;
    assert!(matches!(refused, Err(Error::InvalidArgs(_))), "{refused:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unregistered_host_gets_the_item_back_at_once() {
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let sni = service.fakes.sni.clone();
    eventually("the StatusNotifierItem shows", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;

    // The host's connection stays open, as a panel process's would.
    let panel = service.bus.connect().await;
    let host = Wye1Proxy::new(&panel).await.expect("proxy");
    host.register_tray("plasma-applet")
        .await
        .expect("registers");
    eventually("the item hides for the host", || {
        let sni = sni.clone();
        async move { sni.shown().is_none() }
    })
    .await;

    host.unregister_tray().await.expect("unregisters");
    host.unregister_tray().await.expect("twice is fine");
    eventually("the item comes back without waiting", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_hosts_on_one_connection_each_count_decision_8() {
    // Instances of one host share its connection: removing one of two must
    // not bring the item back beside the other.
    let Some(service) = Service::start(SHOWN).await else {
        return;
    };
    let sni = service.fakes.sni.clone();
    eventually("the StatusNotifierItem shows", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;

    let panel = service.bus.connect().await;
    let host = Wye1Proxy::new(&panel).await.expect("proxy");
    for _ in 0..2 {
        host.register_tray("plasma-applet")
            .await
            .expect("an instance registers");
    }
    eventually("the item hides for the host", || {
        let sni = sni.clone();
        async move { sni.shown().is_none() }
    })
    .await;

    host.unregister_tray().await.expect("one instance leaves");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(sni.shown(), None, "the other instance still shows the tray");

    host.unregister_tray().await.expect("the other leaves");
    eventually("the item comes back without hosts", || {
        let sni = sni.clone();
        async move { sni.shown().is_some() }
    })
    .await;
}
