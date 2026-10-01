//! The picker round trip on a private bus (PIPE-13, PICK-23, PICK-27,
//! PICK-29, PICK-31, PICK-33, PKS-06, PKS-07, IN-06): a fake UI host owns
//! `dev.soldunov.wye.Ui` and records what the service asks of it; the test
//! answers the way the picker would. Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use support::{Service, TWO, eventually};
use wye_api::names::{
    GNOME_BUS_NAME, GNOME_OBJECT_PATH, GTK_BUS_NAME, GTK_OBJECT_PATH, UI_BUS_NAME, UI_OBJECT_PATH,
};
use wye_api::picker::PickerRequest;
use wye_api::proxy::Wye1Proxy;
use wye_api::{Error, context};
use wye_service::run;
use zbus::zvariant::Value;

const URL: &str = "https://example.com/";
const PICKER: &str = "[browsers]\nprimary = { picker = true }\n";

/// One call the fake UI received.
#[derive(Debug, Clone, PartialEq)]
enum Call {
    Show {
        id: String,
        request: Box<PickerRequest>,
    },
    Close {
        id: String,
    },
    Menu,
    Window {
        window: String,
        argument: String,
    },
}

#[derive(Clone, Default)]
struct FakeUi {
    calls: Arc<Mutex<Vec<Call>>>,
}

impl FakeUi {
    fn record(&self, call: Call) {
        self.calls.lock().expect("not poisoned").push(call);
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("not poisoned").clone()
    }

    fn shown(&self) -> Vec<(String, PickerRequest)> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                Call::Show { id, request } => Some((id, *request)),
                _ => None,
            })
            .collect()
    }
}

struct PickerHost(FakeUi);

#[allow(
    clippy::unused_self,
    reason = "the fake serves the whole interface; members it ignores still take their arguments"
)]
#[zbus::interface(name = "dev.soldunov.wye.PickerHost1")]
impl PickerHost {
    fn show_picker(&self, request_id: &str, request: &str) -> zbus::fdo::Result<()> {
        let request = serde_json::from_str(request)
            .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;
        self.0.record(Call::Show {
            id: request_id.to_owned(),
            request: Box::new(request),
        });
        Ok(())
    }

    fn close_picker(&self, request_id: &str) {
        self.0.record(Call::Close {
            id: request_id.to_owned(),
        });
    }

    fn show_menu(&self, menu: &str) {
        let _ = menu;
        self.0.record(Call::Menu);
    }
}

struct Windows(FakeUi);

#[allow(
    clippy::unused_self,
    reason = "the fake serves the whole interface; Quit has nothing to record"
)]
#[zbus::interface(name = "dev.soldunov.wye.Windows1")]
impl Windows {
    fn show_window(&self, window: &str, argument: &str) {
        self.0.record(Call::Window {
            window: window.to_owned(),
            argument: argument.to_owned(),
        });
    }

    fn quit(&self) {}
}

struct BrokenWindows;

#[allow(
    clippy::unused_self,
    reason = "the D-Bus test host always rejects window requests"
)]
#[zbus::interface(name = "dev.soldunov.wye.Windows1")]
impl BrokenWindows {
    fn show_window(&self, window: &str, argument: &str) -> zbus::fdo::Result<()> {
        let _ = (window, argument);
        Err(zbus::fdo::Error::Failed("GTK failed".to_owned()))
    }
}

/// The fake UI host, owning its name on the service's bus.
async fn fake_ui(service: &Service) -> (FakeUi, zbus::Connection) {
    let ui = FakeUi::default();
    let connection = service.bus.connect().await;
    let server = connection.object_server();
    server
        .at(UI_OBJECT_PATH, PickerHost(ui.clone()))
        .await
        .expect("served");
    server
        .at(UI_OBJECT_PATH, Windows(ui.clone()))
        .await
        .expect("served");
    connection.request_name(UI_BUS_NAME).await.expect("name");
    (ui, connection)
}

async fn gnome_picker(service: &Service) -> (FakeUi, zbus::Connection) {
    let ui = FakeUi::default();
    let connection = service.bus.connect().await;
    connection
        .object_server()
        .at(GNOME_OBJECT_PATH, PickerHost(ui.clone()))
        .await
        .expect("served");
    connection.request_name(GNOME_BUS_NAME).await.expect("name");
    (ui, connection)
}

async fn gtk_windows(service: &Service) -> (FakeUi, zbus::Connection) {
    let ui = FakeUi::default();
    let connection = service.bus.connect().await;
    connection
        .object_server()
        .at(GTK_OBJECT_PATH, Windows(ui.clone()))
        .await
        .expect("served");
    connection.request_name(GTK_BUS_NAME).await.expect("name");
    (ui, connection)
}

fn cli() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([(context::ENTRY, Value::from("cli"))])
}

async fn wye(service: &Service) -> Wye1Proxy<'static> {
    Wye1Proxy::new(&service.client).await.expect("proxy")
}

fn two() -> String {
    format!(r#"{{"app":"{TWO}"}}"#)
}

async fn shown_count(ui: &FakeUi, count: usize) {
    eventually("the picker to be shown", || async {
        ui.shown().len() >= count
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gnome_prefers_live_shell_and_falls_back_to_qt_without_losing_a_request() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("ubuntu:GNOME"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (shell, shell_connection) = gnome_picker(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    proxy.toggle_menu().await.expect("Shell menu");
    assert!(shell.calls().contains(&Call::Menu));
    assert!(qt.calls().is_empty(), "shell owns the picker and menu");
    let first = shell.shown().remove(0).0;
    // Let the service's name-owner subscription settle before removing Shell.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    shell_connection
        .release_name(GNOME_BUS_NAME)
        .await
        .expect("released");
    shown_count(&qt, 1).await;
    assert_eq!(qt.shown()[0].0, first, "pending request handed to Qt");
    proxy
        .open_link("https://example.com/new", cli())
        .await
        .expect("routed");
    shown_count(&qt, 2).await;
    let second = qt.shown()[1].0.clone();
    assert!(matches!(
        proxy.picker_cancelled(&first).await,
        Err(Error::NotFound(_))
    ));
    proxy
        .picker_chose(&second, &two(), HashMap::new())
        .await
        .expect("chosen");
    assert_eq!(
        service.launched(),
        [vec![
            "fake-two".to_owned(),
            "https://example.com/new".to_owned()
        ]]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gnome_windows_use_gtk_and_qt_when_gtk_is_missing() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("GNOME"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, gtk_connection) = gtk_windows(&service).await;
    let proxy = wye(&service).await;
    proxy
        .show_window("settings", "general")
        .await
        .expect("GTK window");
    assert_eq!(gtk.calls().len(), 1);
    assert!(qt.calls().is_empty());
    gtk_connection
        .release_name(GTK_BUS_NAME)
        .await
        .expect("released");
    proxy.show_window("history", "").await.expect("Qt window");
    assert_eq!(qt.calls().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_gtk_does_not_silently_switch_to_qt() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("GNOME"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let gtk_connection = service.bus.connect().await;
    gtk_connection
        .object_server()
        .at(GTK_OBJECT_PATH, BrokenWindows)
        .await
        .expect("served");
    gtk_connection
        .request_name(GTK_BUS_NAME)
        .await
        .expect("name");
    let result = wye(&service).await.show_window("settings", "").await;
    assert!(result.is_err(), "GTK failure must be reported");
    assert!(
        qt.calls().is_empty(),
        "Qt must not hide an installed GTK failure"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kde_keeps_qt_even_when_gnome_hosts_are_present() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("KDE"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (shell, _shell_connection) = gnome_picker(&service).await;
    let (gtk, _gtk_connection) = gtk_windows(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&qt, 1).await;
    proxy.show_window("settings", "").await.expect("Qt window");
    proxy.toggle_menu().await.expect("Qt menu");
    assert!(qt.calls().contains(&Call::Menu));
    assert!(shell.calls().is_empty());
    assert!(gtk.calls().is_empty());
}

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
