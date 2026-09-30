//! The `StatusNotifierItem` and its `DBusMenu` over a private bus (TRAY-01,
//! TRAY-07, TRAY-11, TRAY-13, ONB-11).
//!
//! ksni connects to the session bus named by `DBUS_SESSION_BUS_ADDRESS`, so
//! the item runs in a child process (this test binary again, running the
//! ignored `child_shows_the_item`) with that variable pointing at a private
//! bus. The parent plays the `StatusNotifierWatcher` and the tray host: it
//! reads the item's properties and menu, clicks an item, and the child
//! reports the event it got. Skips without `dbus-daemon`.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use wye_api::tray::{TrayIcon, TrayItem, TrayItemKind, TrayMenu, TrayOverlay};
use wye_service::platform::sni::{KsniNotifier, WARNING_OVERLAY};
use wye_service::platform::{StatusNotifier as _, TrayEvent};
use zbus::message::Header;
use zbus::zvariant::{OwnedValue, Value};

/// The child's environment variable holding the private bus address.
const BUS_VARIABLE: &str = "DBUS_SESSION_BUS_ADDRESS";

/// What the child prints once an item is clicked.
const ACTIVATED: &str = "ACTIVATED ";

const PATIENCE: Duration = Duration::from_secs(10);

fn item(id: &str, kind: TrayItemKind, label: &str) -> TrayItem {
    TrayItem {
        id: id.to_owned(),
        kind,
        label: label.to_owned(),
        icon: None,
        shortcut: None,
        enabled: true,
        checked: false,
        children: Vec::new(),
    }
}

/// A small menu with every kind of entry.
fn menu() -> TrayMenu {
    TrayMenu {
        icon: TrayIcon::Picker,
        overlay: Some(TrayOverlay::Warning),
        visible: true,
        items: vec![
            item("primary-header", TrayItemKind::Header, "Primary Browser"),
            TrayItem {
                checked: true,
                shortcut: Some("P".to_owned()),
                ..item("primary:picker", TrayItemKind::Radio, "Picker")
            },
            item("primary:0", TrayItemKind::Radio, "Firefox"),
            item("separator:0", TrayItemKind::Separator, ""),
            TrayItem {
                shortcut: Some("Ctrl+,".to_owned()),
                ..item("settings", TrayItemKind::Action, "Settings…")
            },
            TrayItem {
                children: vec![item("about", TrayItemKind::Action, "About Wye")],
                ..item("more", TrayItemKind::Submenu, "More")
            },
        ],
    }
}

/// The half that runs in the child: show the item, report one click.
#[tokio::test]
#[ignore = "run by the_status_notifier_item_serves_the_menu in a child process"]
async fn child_shows_the_item() {
    if std::env::var_os(BUS_VARIABLE).is_none() {
        return;
    }
    let tray = KsniNotifier::new(Duration::ZERO);
    let mut events = tray.events();
    tray.show(&menu()).await.expect("the item shows");
    println!("SHOWN");
    loop {
        match tokio::time::timeout(PATIENCE, events.recv()).await {
            Ok(Ok(TrayEvent::Activated(id))) => {
                println!("{ACTIVATED}{id}");
                break;
            }
            Ok(Ok(TrayEvent::AboutToShow)) => println!("ABOUT TO SHOW"),
            other => panic!("no click arrived: {other:?}"),
        }
    }
    tray.hide().await;
}

/// `org.kde.StatusNotifierWatcher`, recording who registered.
#[derive(Default)]
struct Watcher {
    registered: Arc<Mutex<Vec<(String, String)>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
#[allow(
    clippy::unused_self,
    clippy::needless_pass_by_value,
    reason = "the signatures are the StatusNotifierWatcher specification's"
)]
impl Watcher {
    fn register_status_notifier_item(&self, #[zbus(header)] header: Header<'_>, service: String) {
        let sender = header.sender().map(ToString::to_string).unwrap_or_default();
        self.registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((service, sender));
    }

    fn register_status_notifier_host(&self, service: String) {
        // Only items matter here; a host registering is fine.
        drop(service);
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// A private session bus, killed when dropped.
struct Bus {
    daemon: Child,
    address: String,
}

impl Bus {
    fn start() -> Option<Self> {
        let program = find_on_path("dbus-daemon")?;
        let mut daemon = Command::new(program)
            .args(["--session", "--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon starts");
        let stdout = daemon.stdout.take().expect("piped");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("an address");
        Some(Self {
            daemon,
            address: address.trim().to_owned(),
        })
    }

    async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .expect("address parses")
            .build()
            .await
            .expect("connected")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let found = std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file());
    if found.is_none() {
        eprintln!("skipping: {program} is not on PATH");
    }
    found
}

/// One `DBusMenu` layout node: id, properties, children.
type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

fn node(value: OwnedValue) -> Node {
    Node::try_from(value).expect("a layout node")
}

fn text(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| String::try_from(value.clone()).ok())
}

/// Every node under `root`, depth first, as (id, label, properties).
fn flatten(root: Node) -> Vec<(i32, String, HashMap<String, OwnedValue>)> {
    let (id, properties, children) = root;
    let label = text(&properties, "label").unwrap_or_default();
    let mut all = vec![(id, label, properties)];
    for child in children {
        all.extend(flatten(node(child)));
    }
    all
}

/// Serve the watcher on `bus`; returns who registered.
async fn start_watcher(bus: &Bus) -> (zbus::Connection, Arc<Mutex<Vec<(String, String)>>>) {
    let connection = bus.connect().await;
    let watcher = Watcher::default();
    let registered = Arc::clone(&watcher.registered);
    connection
        .object_server()
        .at("/StatusNotifierWatcher", watcher)
        .await
        .expect("served");
    connection
        .request_name("org.kde.StatusNotifierWatcher")
        .await
        .expect("named");
    (connection, registered)
}

/// The child process and the lines it prints.
struct ChildItem {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
    reader: std::thread::JoinHandle<()>,
}

impl ChildItem {
    fn spawn(bus: &Bus) -> Self {
        let exe = std::env::current_exe().expect("the test binary");
        let mut child = Command::new(exe)
            .args([
                "--exact",
                "child_shows_the_item",
                "--ignored",
                "--nocapture",
            ])
            .env(BUS_VARIABLE, &bus.address)
            .stdout(Stdio::piped())
            .spawn()
            .expect("the child starts");
        let out = child.stdout.take().expect("piped");
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let reader = {
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    lines
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(line);
                }
            })
        };
        Self {
            child,
            lines,
            reader,
        }
    }

    /// Wait for the child to exit; whether it succeeded and what it printed.
    async fn finish(self) -> (bool, Vec<String>) {
        let mut child = self.child;
        let status = tokio::task::spawn_blocking(move || child.wait())
            .await
            .expect("joined")
            .expect("the child ran");
        self.reader.join().expect("the reader ran");
        let lines = self
            .lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        (status.success(), lines)
    }
}

/// The bus name the item registered with the watcher.
async fn registered_service(registered: &Mutex<Vec<(String, String)>>) -> String {
    tokio::time::timeout(PATIENCE, async {
        loop {
            let found = registered
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .first()
                .cloned();
            if let Some((service, _)) = found {
                return service;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the item registered with the watcher")
}

/// TRAY-02, TRAY-07, ONB-11: the item's own properties.
async fn check_item(host: &zbus::Connection, service: &str) {
    let item = zbus::Proxy::new(
        host,
        service,
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await
    .expect("item proxy");
    let icon: String = item.get_property("IconName").await.expect("IconName");
    assert_eq!(icon, "dev.soldunov.wye-picker-symbolic", "TRAY-02");
    let overlay: String = item.get_property("OverlayIconName").await.expect("overlay");
    assert_eq!(overlay, WARNING_OVERLAY, "ONB-11");
    let is_menu: bool = item.get_property("ItemIsMenu").await.expect("ItemIsMenu");
    assert!(is_menu, "TRAY-07: a primary click opens the menu");
}

/// The menu's layout, checked; returns the ID of "Settings…".
async fn check_layout(menu: &zbus::Proxy<'_>) -> i32 {
    let (_revision, root): (u32, Node) = menu
        .call("GetLayout", &(0_i32, -1_i32, Vec::<String>::new()))
        .await
        .expect("GetLayout");
    let entries = flatten(root);
    let find = |label: &str| {
        entries
            .iter()
            .find(|(_, candidate, _)| candidate == label)
            .unwrap_or_else(|| panic!("{label} missing from the menu"))
    };
    for expected in ["Firefox", "More", "About Wye"] {
        find(expected);
    }
    let (_, _, picker) = find("Picker");
    assert_eq!(
        text(picker, "toggle-type").as_deref(),
        Some("radio"),
        "TRAY-11"
    );
    let (_, _, header) = find("Primary Browser");
    let enabled = header
        .get("enabled")
        .and_then(|value| bool::try_from(value.clone()).ok());
    assert_eq!(enabled, Some(false), "a header cannot be chosen");
    let (settings, _, properties) = find("Settings…");
    assert!(
        properties.contains_key("shortcut"),
        "TRAY-13: {properties:?}"
    );
    *settings
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_status_notifier_item_serves_the_menu() {
    let Some(bus) = Bus::start() else {
        return;
    };
    let (_watcher, registered) = start_watcher(&bus).await;
    let child = ChildItem::spawn(&bus);
    let service = registered_service(&registered).await;
    assert!(
        service.starts_with("org.kde.StatusNotifierItem-"),
        "{service}"
    );

    let host = bus.connect().await;
    check_item(&host, &service).await;
    let menu = zbus::Proxy::new(
        &host,
        service.as_str(),
        "/MenuBar",
        "com.canonical.dbusmenu",
    )
    .await
    .expect("menu proxy");
    let settings = check_layout(&menu).await;
    let _: bool = menu
        .call("AboutToShow", &(0_i32,))
        .await
        .expect("AboutToShow");
    let (): () = menu
        .call("Event", &(settings, "clicked", Value::from(0_i32), 0_u32))
        .await
        .expect("Event");

    let (succeeded, lines) = child.finish().await;
    assert!(succeeded, "the child failed: {lines:?}");
    assert!(
        lines.iter().any(|line| line == "ABOUT TO SHOW"),
        "TRAY-10: {lines:?}"
    );
    let clicked = format!("{ACTIVATED}settings");
    assert!(lines.contains(&clicked), "{lines:?}");
}
