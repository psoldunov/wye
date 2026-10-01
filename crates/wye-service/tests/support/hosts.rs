//! Fake picker and window hosts for the picker round trip (PIPE-13,
//! ADV-12): each owns a host's name on the service's private bus and
//! records what the service asks of it.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wye_api::context;
use wye_api::names::{
    GNOME_BUS_NAME, GNOME_OBJECT_PATH, GTK_BUS_NAME, GTK_OBJECT_PATH, UI_BUS_NAME, UI_OBJECT_PATH,
};
use wye_api::picker::PickerRequest;
use wye_api::proxy::Wye1Proxy;
use zbus::zvariant::Value;

use super::{Service, TWO, eventually};

pub const URL: &str = "https://example.com/";
pub const PICKER: &str = "[browsers]\nprimary = { picker = true }\n";

/// GNOME Shell's own name, which makes the session a GNOME one (ADV-12).
const SHELL_NAME: &str = "org.gnome.Shell";

/// One call a fake host received.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
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
pub struct FakeUi {
    calls: Arc<Mutex<Vec<Call>>>,
}

impl FakeUi {
    fn record(&self, call: Call) {
        self.calls.lock().expect("not poisoned").push(call);
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("not poisoned").clone()
    }

    pub fn shown(&self) -> Vec<(String, PickerRequest)> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                Call::Show { id, request } => Some((id, *request)),
                _ => None,
            })
            .collect()
    }

    /// Whether the host was asked to close request `id`.
    pub fn closed(&self, id: &str) -> bool {
        self.calls().contains(&Call::Close { id: id.to_owned() })
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

pub struct BrokenWindows;

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

/// The fake Qt host, owning its name on the service's bus.
pub async fn fake_ui(service: &Service) -> (FakeUi, zbus::Connection) {
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

/// The GNOME Shell extension's picker.
pub async fn gnome_picker(service: &Service) -> (FakeUi, zbus::Connection) {
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

/// A GTK host serving `interface` at its path.
async fn gtk_serving(
    service: &Service,
    interface: impl zbus::object_server::Interface,
) -> zbus::Connection {
    let connection = service.bus.connect().await;
    connection
        .object_server()
        .at(GTK_OBJECT_PATH, interface)
        .await
        .expect("served");
    connection.request_name(GTK_BUS_NAME).await.expect("name");
    connection
}

/// A GTK host that serves the windows only.
pub async fn gtk_windows(service: &Service) -> (FakeUi, zbus::Connection) {
    let ui = FakeUi::default();
    let connection = gtk_serving(service, Windows(ui.clone())).await;
    (ui, connection)
}

/// The GTK host once it also serves the picker and the tray popup.
pub async fn gtk_host(service: &Service) -> (FakeUi, zbus::Connection) {
    let (ui, connection) = gtk_windows(service).await;
    connection
        .object_server()
        .at(GTK_OBJECT_PATH, PickerHost(ui.clone()))
        .await
        .expect("served");
    (ui, connection)
}

/// A GTK host whose `Windows1` always fails.
pub async fn broken_gtk_windows(service: &Service) -> zbus::Connection {
    gtk_serving(service, BrokenWindows).await
}

/// GNOME Shell itself: owning `org.gnome.Shell` makes the session a GNOME
/// one for the service (ADV-12), whatever `XDG_CURRENT_DESKTOP` says.
pub async fn gnome_session(service: &Service) -> zbus::Connection {
    let connection = service.bus.connect().await;
    connection.request_name(SHELL_NAME).await.expect("name");
    connection
}

/// The picker configuration with `advanced.frontend` set (ADV-12).
pub fn with_frontend(frontend: &str) -> String {
    format!("{PICKER}\n[advanced]\nfrontend = \"{frontend}\"\n")
}

pub fn cli() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([(context::ENTRY, Value::from("cli"))])
}

pub async fn wye(service: &Service) -> Wye1Proxy<'static> {
    Wye1Proxy::new(&service.client).await.expect("proxy")
}

pub fn two() -> String {
    format!(r#"{{"app":"{TWO}"}}"#)
}

pub async fn shown_count(ui: &FakeUi, count: usize) {
    eventually("the picker to be shown", || async {
        ui.shown().len() >= count
    })
    .await;
}

/// Whether `ui` was asked for a window.
pub fn opened_a_window(ui: &FakeUi) -> bool {
    ui.calls()
        .iter()
        .any(|call| matches!(call, Call::Window { .. }))
}

/// Release `name` from `connection` until `done` holds. The service may
/// not follow the name yet when it is first released, so the name is taken
/// and released again until the service reacts.
pub async fn release_until<F: Future<Output = bool>>(
    connection: &zbus::Connection,
    name: &str,
    what: &str,
    done: impl Fn() -> F,
) {
    for _ in 0..10 {
        // Already the owner the first time: taking it again changes nothing.
        connection.request_name(name).await.expect("name");
        connection.release_name(name).await.expect("released");
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while tokio::time::Instant::now() < deadline {
            if done().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    panic!("timed out waiting for {what}");
}
