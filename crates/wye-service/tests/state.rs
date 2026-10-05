//! Status, UI state and the default browser on a private bus (SET-08,
//! BLK-09, ONB-06, DEF-02, DEF-03, DEF-05, ONB-10, ONB-11,
//! DLG-ABT-02). Skips without `dbus-daemon`.

mod support;

use support::{ONE, Service, TWO, WYE, eventually};
use wye_api::Error;

const MIMEAPPS: &str = "config/mimeapps.list";
const KDEGLOBALS: &str = "config/kdeglobals";
const STATE: &str = "state/wye/state.toml";

fn listing(id: &str) -> String {
    format!("[Default Applications]\nx-scheme-handler/http={id}\nx-scheme-handler/https={id}\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set08_ui_state_is_kept_in_the_state_file() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    wye.update_ui_state(r#"{"lastPage": "rules", "dismissedCallouts": ["apps-read"]}"#)
        .await
        .expect("saved");
    let ui = service.status().await.ui_state;
    assert_eq!(ui.last_page.as_deref(), Some("rules"));
    assert_eq!(ui.dismissed_callouts, ["apps-read"]);
    let file = service.desktop.read(STATE);
    assert!(file.contains("last-settings-page = \"rules\""), "{file}");

    let invalid = wye.update_ui_state(r#"{"onboardingDone": 3}"#).await;
    assert!(matches!(invalid, Err(Error::InvalidArgs(_))), "{invalid:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_the_configuration_file() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let config = service.status().await.config;
    assert!(config.path.ends_with("wye/config.toml"), "{}", config.path);
    assert!(config.writable && config.lossless && config.error.is_none());
    let report = service
        .wye()
        .await
        .get_troubleshooting()
        .await
        .expect("DLG-ABT-02");
    assert!(report.contains("Held keys: fake"), "{report}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def02_def05_make_default_and_give_it_back_with_kdeglobals() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    desktop.write(MIMEAPPS, &listing(ONE));
    desktop.write(
        KDEGLOBALS,
        "[General]\nBrowserApplication=fake-one.desktop\n",
    );
    service.ctx.set_environment(desktop.environment_on("KDE"));
    let wye = service.wye().await;

    wye.make_default().await.expect("DEF-02");
    assert!(
        desktop
            .read(MIMEAPPS)
            .contains(&format!("x-scheme-handler/https={WYE}"))
    );
    assert!(
        desktop
            .read(KDEGLOBALS)
            .contains(&format!("BrowserApplication={WYE}"))
    );
    let status = service.status().await.default_browser;
    assert!(status.is_default);
    assert_eq!(
        status.previous.map(|app| app.name),
        Some("Fake One".to_owned())
    );
    let state = desktop.read(STATE);
    assert!(
        state.contains("previous-default-browser = \"fake-one.desktop\""),
        "{state}"
    );
    assert!(state.contains("previous-kdeglobals-browser"), "{state}");

    wye.stop_being_default().await.expect("DEF-05");
    assert!(
        desktop
            .read(MIMEAPPS)
            .contains(&format!("x-scheme-handler/https={ONE}"))
    );
    assert!(
        desktop
            .read(KDEGLOBALS)
            .contains(&format!("BrowserApplication={ONE}"))
    );
    let status = service.status().await.default_browser;
    assert!(!status.is_default);
    assert_eq!(status.current.map(|app| app.id), Some(ONE.to_owned()));
    assert!(status.kept_current, "giving it back is not a takeover");
}

/// With an unreadable state file `MakeDefault` fails before it changes
/// anything, so the browser it would replace is never left unrecorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def05_make_default_with_an_unreadable_state_file_changes_nothing() {
    let shown = format!("[[browsers.shown]]\ntarget = {{ app = \"{ONE}\" }}\n");
    let Some(service) = Service::start(&shown).await else {
        return;
    };
    let desktop = &service.desktop;
    // The first scan writes the state; let it finish so it cannot replace
    // the invalid file below.
    service.wye().await.get_targets().await.expect("GetTargets");
    eventually("the first scan is recorded", || async {
        desktop.read(STATE).contains(TWO)
    })
    .await;
    desktop.install_wye();
    desktop.write(MIMEAPPS, &listing(ONE));
    let kde = "[General]\nBrowserApplication=fake-one.desktop\n";
    desktop.write(KDEGLOBALS, kde);
    let invalid = "previous-default-browser = 3\n";
    desktop.write(STATE, invalid);
    service.ctx.set_environment(desktop.environment_on("KDE"));

    let refused = service.wye().await.make_default().await;
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(desktop.read(MIMEAPPS), listing(ONE));
    assert_eq!(desktop.read(KDEGLOBALS), kde);
    assert_eq!(desktop.read(STATE), invalid);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def05_without_a_remembered_browser_nothing_changes() {
    let Some(service) = Service::start("").await else {
        return;
    };
    service.desktop.write(MIMEAPPS, &listing(WYE));
    let refused = service.wye().await.stop_being_default().await;
    assert!(matches!(refused, Err(Error::NotFound(_))), "{refused:?}");
    assert_eq!(service.desktop.read(MIMEAPPS), listing(WYE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def03_a_takeover_is_notified_once_with_both_answers() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    let wye = service.wye().await;
    // `MakeDefault` records Wye as the default before it returns, so the
    // takeover right after it counts without waiting for the watcher.
    wye.make_default().await.expect("DEF-02");
    desktop.replace(MIMEAPPS, &listing(ONE));
    eventually("the takeover notification", || async {
        !service.fakes.notifier.shown().is_empty()
    })
    .await;
    let shown = service.fakes.notifier.shown();
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(
        shown[0].body.contains("Fake One took over"),
        "{}",
        shown[0].body
    );
    let actions: Vec<&str> = shown[0]
        .actions
        .iter()
        .map(|(_, label)| label.as_str())
        .collect();
    assert_eq!(actions, ["Make Wye Default", "Keep Fake One"]);

    // TRAY-18: until answered, the tray warns and offers to fix it.
    let menu = tray(&service.wye().await).await;
    assert!(menu.overlay.is_some());
    assert_eq!(menu.items[0].id, "make-default");

    // ONB-11: "Keep Fake One" stops the warning, in the tray too.
    service.fakes.notifier.press(1, "wye-keep-default");
    eventually("the kept default", || async {
        service.status().await.default_browser.kept_current
    })
    .await;
    // A fresh proxy: the long-lived one caches `Tray` until its signal.
    let menu = tray(&service.wye().await).await;
    assert_eq!(menu.overlay, None, "ONB-11: dismissed with Keep");
    assert!(
        menu.items.iter().all(|item| item.id != "make-default"),
        "{:?}",
        menu.items
    );
}

/// DEF-03, ONB-11: a takeover fixed another way (here the tray's "Make Wye
/// Default Browser") withdraws its notification, and its stale "Keep"
/// button no longer records a kept default.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def03_a_fixed_takeover_withdraws_its_notification() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    let wye = service.wye().await;
    wye.make_default().await.expect("DEF-02");
    desktop.replace(MIMEAPPS, &listing(ONE));
    eventually("the takeover notification", || async {
        !service.fakes.notifier.shown().is_empty()
    })
    .await;

    wye.activate_tray_item("make-default")
        .await
        .expect("TRAY-18");
    assert!(service.status().await.default_browser.is_default);
    eventually("the notification withdrawn", || async {
        service.fakes.notifier.closed() == [1]
    })
    .await;

    service.fakes.notifier.press(1, "wye-keep-default");
    assert!(service.status().await.default_browser.is_default);
    // The same app taking over again is notified again, which a kept
    // default would have silenced: the stale press recorded nothing. The
    // button is answered at once, the watcher only after its quiet time.
    desktop.replace(MIMEAPPS, &listing(ONE));
    eventually("a second takeover notification", || async {
        service.fakes.notifier.shown().len() == 2
    })
    .await;
    let state = desktop.read(STATE);
    assert!(!state.contains("kept-default"), "{state}");
}

/// DEF-07: while Wye is the default browser, "Also open local HTML files"
/// changes `mimeapps.list` at once; while it is not, nothing changes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def07_the_html_switch_follows_while_wye_is_the_default() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    desktop.write(MIMEAPPS, &listing(ONE));
    let wye = service.wye().await;
    let html = |on: bool| format!(r#"{{"general": {{"open-local-html": {on}}}}}"#);

    wye.update_config(&html(true), 0).await.expect("saved");
    assert_eq!(desktop.read(MIMEAPPS), listing(ONE), "not the default");
    wye.update_config(&html(false), 0).await.expect("saved");

    wye.make_default().await.expect("DEF-02");
    assert!(!desktop.read(MIMEAPPS).contains("text/html"));
    wye.update_config(&html(true), 0).await.expect("saved");
    let text = desktop.read(MIMEAPPS);
    assert!(text.contains(&format!("text/html={WYE}\n")), "{text}");
    assert!(
        text.contains(&format!("application/xhtml+xml={WYE}\n")),
        "{text}"
    );

    // Off again: HTML files go back to the browser Wye replaced.
    wye.update_config(&html(false), 0).await.expect("saved");
    let text = desktop.read(MIMEAPPS);
    assert!(text.contains(&format!("text/html={ONE}\n")), "{text}");
    assert!(!text.contains(&format!("{WYE};")), "{text}");
    assert!(
        text.contains(&format!("x-scheme-handler/https={WYE}")),
        "{text}"
    );
}

fn html_switch(on: bool) -> String {
    format!(r#"{{"general": {{"open-local-html": {on}}}}}"#)
}

/// DEF-07: HTML files left with Wye by an earlier "on" go back when Wye is
/// made the default again with the switch off.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def07_make_default_with_the_switch_off_releases_old_html_keys() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    desktop.write(MIMEAPPS, &listing(ONE));
    let wye = service.wye().await;
    wye.make_default().await.expect("DEF-02");
    wye.update_config(&html_switch(true), 0)
        .await
        .expect("saved");
    assert!(
        desktop
            .read(MIMEAPPS)
            .contains(&format!("text/html={WYE}\n"))
    );

    // Another browser takes the links but not the HTML types.
    let taken = desktop.read(MIMEAPPS).replace(
        &format!("x-scheme-handler/http={WYE}\nx-scheme-handler/https={WYE}\n"),
        &format!("x-scheme-handler/http={ONE}\nx-scheme-handler/https={ONE}\n"),
    );
    assert!(taken.contains(&format!("https={ONE}")), "{taken}");
    desktop.replace(MIMEAPPS, &taken);
    wye.update_config(&html_switch(false), 0)
        .await
        .expect("saved");
    assert_eq!(desktop.read(MIMEAPPS), taken, "not the default: unchanged");

    wye.make_default().await.expect("DEF-02");
    let text = desktop.read(MIMEAPPS);
    assert!(text.contains(&format!("text/html={ONE}\n")), "{text}");
    assert!(
        text.contains(&format!("application/xhtml+xml={ONE}\n")),
        "{text}"
    );
    assert!(!text.contains(&format!("{WYE};")), "{text}");
    assert!(
        text.contains(&format!("x-scheme-handler/https={WYE}")),
        "{text}"
    );
}

/// DEF-07: on KDE the switch follows `mimeapps.list`, even while Plasma's
/// own setting (which says nothing about HTML files) names another browser.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn def07_the_html_switch_follows_mimeapps_on_kde() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let desktop = &service.desktop;
    desktop.install_wye();
    desktop.write(MIMEAPPS, &listing(WYE));
    desktop.write(
        KDEGLOBALS,
        "[General]\nBrowserApplication=fake-one.desktop\n",
    );
    service.ctx.set_environment(desktop.environment_on("KDE"));
    let wye = service.wye().await;

    wye.update_config(&html_switch(true), 0)
        .await
        .expect("saved");
    let text = desktop.read(MIMEAPPS);
    assert!(text.contains(&format!("text/html={WYE}\n")), "{text}");
}

async fn tray(wye: &wye_api::proxy::Wye1Proxy<'_>) -> wye_api::tray::TrayMenu {
    let text = wye.tray().await.expect("the Tray property");
    serde_json::from_str(&text).expect("Tray is JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onb11_keep_current_default_remembers_the_app() {
    let Some(service) = Service::start("").await else {
        return;
    };
    service.desktop.write(MIMEAPPS, &listing(ONE));
    service
        .wye()
        .await
        .keep_current_default()
        .await
        .expect("kept");
    assert!(
        service
            .desktop
            .read(STATE)
            .contains("kept-default = \"fake-one.desktop\"")
    );
    assert!(service.status().await.default_browser.kept_current);
}

/// KEY-06, DLG-ABT-02: `Status.capabilities` follow the integrations, so a
/// change of them (the probes detected after start) is announced.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_is_announced_when_the_integrations_change() {
    use futures_lite::StreamExt as _;

    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    service.status().await;
    let mut changes = wye.receive_status_changed().await;
    service.ctx.update_platform(Clone::clone);
    tokio::time::timeout(std::time::Duration::from_secs(5), changes.next())
        .await
        .expect("Status announced")
        .expect("a change");
}
