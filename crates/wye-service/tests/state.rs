//! Status, UI state and the default browser on a private bus (SET-08,
//! BLK-09, ONB-06, DEF-02, DEF-03, DEF-05, ONB-10, ONB-11,
//! DLG-ABT-02). Skips without `dbus-daemon`.

mod support;

use support::{ONE, Service, WYE, eventually};
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
    wye.make_default().await.expect("DEF-02");
    // Let the watcher see Wye as the default before the takeover.
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

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

    // ONB-11: "Keep Fake One" stops the warning.
    service.fakes.notifier.press(1, "wye-keep-default");
    eventually("the kept default", || async {
        service.status().await.default_browser.kept_current
    })
    .await;
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
