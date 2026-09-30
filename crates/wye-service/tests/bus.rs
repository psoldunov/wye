//! The service on a private bus, driven through the `wye-api` proxies.
//!
//! Every member of the contract must answer: with a real reply, or with
//! `dev.soldunov.wye.Error.NotImplemented` while its topic is still a stub.
//! What the link path does is tested in `link.rs`.
//! Skips (and says so) when `dbus-daemon` is not on `PATH`.

mod support;

use std::collections::HashMap;
use std::fmt::Debug;

use support::Service;
use wye_api::proxy::{ApplicationProxy, KWin1Proxy, Wye1Proxy};
use wye_api::status::Status;
use wye_api::tray::TrayMenu;
use wye_api::{Error, context, json};
use wye_service::platform::fake::FakePlatform;
use wye_service::{ServiceContext, bus, is_already_running, run};
use zbus::zvariant::Value;

const URL: &str = "https://example.com/";

async fn harness() -> Option<Service> {
    Service::start("").await
}

/// A real reply, `NotImplemented`, or `Unavailable` (no UI host on the
/// private bus); anything else fails the test.
fn answered<T: Debug>(member: &str, reply: Result<T, Error>) {
    match reply {
        Ok(_) | Err(Error::NotImplemented(_) | Error::Unavailable(_)) => {}
        Err(other) => panic!("{member} answered {other}"),
    }
}

fn link_context() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([
        (context::ENTRY, Value::from(context::Entry::Cli.as_str())),
        (context::SOURCE_PID, Value::from(1_u32)),
        (context::HELD_KNOWN, Value::from(false)),
    ])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_wye1_method_answers() {
    let Some(harness) = harness().await else {
        return;
    };
    let wye = Wye1Proxy::new(&harness.client).await.expect("proxy");

    answered("OpenLink", wye.open_link(URL, link_context()).await);
    answered("OpenClipboard", wye.open_clipboard(false).await);
    answered("ClipboardHasUrl", wye.clipboard_has_url().await);
    answered("TestLink", wye.test_link(URL, link_context()).await);
    answered("PreviewPicker", wye.preview_picker().await);
    answered(
        "PickerChose",
        wye.picker_chose("1", r#"{"picker":true}"#, HashMap::new())
            .await,
    );
    answered("PickerCancelled", wye.picker_cancelled("1").await);
    answered("PickerAction", wye.picker_action("1", "copy-link").await);
    answered("GetConfig", wye.get_config().await);
    answered("UpdateConfig", wye.update_config("{}", 0).await);
    answered("SetPrimary", wye.set_primary(r#"{"picker":true}"#).await);
    answered("GetDefaults", wye.get_defaults("picker.keys").await);
    answered("GetTargets", wye.get_targets().await);
    answered("GetApps", wye.get_apps(true).await);
    answered("GetServices", wye.get_services().await);
    answered("GetExpansionCatalogue", wye.get_expansion_catalogue().await);
    answered("Rescan", wye.rescan().await);
    answered("MakeDefault", wye.make_default().await);
    answered("StopBeingDefault", wye.stop_being_default().await);
    answered("KeepCurrentDefault", wye.keep_current_default().await);
    answered("ExportRules", wye.export_rules().await);
    answered("ImportRules", wye.import_rules("").await);
    answered("GetScript", wye.get_script("global").await);
    answered("SetScript", wye.set_script("global", "").await);
    answered("RunScript", wye.run_script("", URL, HashMap::new()).await);
    answered("GetHistory", wye.get_history().await);
    answered("ClearHistory", wye.clear_history().await);
    answered("DeleteHistoryEntry", wye.delete_history_entry(1).await);
    answered(
        "ReopenHistoryEntry",
        wye.reopen_history_entry(1, "picker").await,
    );
    answered("GetShortcuts", wye.get_shortcuts().await);
    answered("SetShortcut", wye.set_shortcut("toggle-menu", "").await);
    answered("ConfigureShortcuts", wye.configure_shortcuts().await);
    answered("UpdateUiState", wye.update_ui_state("{}").await);
    answered("ShowWindow", wye.show_window("settings", "").await);
    answered("ToggleMenu", wye.toggle_menu().await);
    answered("RegisterTray", wye.register_tray("plasma-applet").await);
    answered("GetTroubleshooting", wye.get_troubleshooting().await);

    // Last: it stops the service.
    wye.quit().await.expect("Quit answers");
    assert!(
        harness.ctx.shutdown_requested(),
        "TRAY-17: Quit stops the service"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stub_answers_with_the_not_implemented_error() {
    let Some(harness) = harness().await else {
        return;
    };
    let wye = Wye1Proxy::new(&harness.client).await.expect("proxy");
    match wye.get_history().await {
        Err(Error::NotImplemented(message)) => assert!(message.contains("GetHistory"), "{message}"),
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn properties_answer_with_valid_payloads() {
    let Some(harness) = harness().await else {
        return;
    };
    let wye = Wye1Proxy::new(&harness.client).await.expect("proxy");

    assert_eq!(
        wye.version().await.expect("Version"),
        env!("CARGO_PKG_VERSION")
    );
    let tray: TrayMenu = json::decode("Tray", &wye.tray().await.expect("Tray")).expect("TrayMenu");
    assert_eq!(tray, TrayMenu::default());
    let status: Status =
        json::decode("Status", &wye.status().await.expect("Status")).expect("Status JSON");
    assert_eq!(status.capabilities.held_keys.as_deref(), Some("fake"));
    assert_eq!(wye.config_revision().await.expect("ConfigRevision"), 0);
    assert_eq!(wye.history_revision().await.expect("HistoryRevision"), 0);
    assert_eq!(
        wye.inventory_revision().await.expect("InventoryRevision"),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn topics_can_announce_changes() {
    let Some(harness) = harness().await else {
        return;
    };
    for property in [
        bus::Property::Tray,
        bus::Property::Status,
        bus::Property::ConfigRevision,
        bus::Property::HistoryRevision,
        bus::Property::InventoryRevision,
    ] {
        bus::property_changed(&harness.ctx, property)
            .await
            .unwrap_or_else(|error| panic!("{property:?}: {error}"));
    }
    bus::menu_requested(&harness.ctx)
        .await
        .expect("MenuRequested");
    bus::script_file_changed(&harness.ctx, "global")
        .await
        .expect("ScriptFileChanged");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_and_kwin_interfaces_answer() {
    let Some(harness) = harness().await else {
        return;
    };
    let application = ApplicationProxy::new(&harness.client).await.expect("proxy");
    answered(
        "Activate",
        application
            .activate(HashMap::new())
            .await
            .map_err(Error::from),
    );
    answered(
        "Open",
        application
            .open(&[URL], HashMap::new())
            .await
            .map_err(Error::from),
    );
    answered(
        "ActivateAction",
        application
            .activate_action("settings", &[], HashMap::new())
            .await
            .map_err(Error::from),
    );

    let kwin = KWin1Proxy::new(&harness.client).await.expect("proxy");
    answered(
        "KWin1.Report",
        kwin.report("nonce", 10, 20, "DP-1", 42, "firefox", "firefox")
            .await,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_service_refuses_to_start() {
    let Some(harness) = harness().await else {
        return;
    };
    let second = ServiceContext::new(FakePlatform::new().platform());
    let error = run::start(&harness.client, &second)
        .await
        .expect_err("the name is taken");
    assert!(is_already_running(&error), "{error:#}");
}
