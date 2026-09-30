//! The real session integrations against fake servers on private buses:
//! notifications (PIPE-02, LAUNCH-07), lock state (PKS-05, PKS-07) and
//! systemd scopes (LAUNCH-06). Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::PrivateBus;
use wye_service::platform::lock::SessionLockMonitor;
use wye_service::platform::notify::DesktopNotifier;
use wye_service::platform::scope::SystemdScopes;
use wye_service::platform::{
    LaunchCommand, LockMonitor as _, Notification, Notifier as _, ScopeManager as _,
};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const WAIT: Duration = Duration::from_secs(5);

/// What the fake notification server received.
#[derive(Debug, Clone)]
struct Received {
    app_name: String,
    replaces_id: u32,
    app_icon: String,
    body: String,
    expire_timeout: i32,
    summary: String,
    actions: Vec<String>,
    desktop_entry: Option<String>,
}

#[derive(Default)]
struct FakeNotifications {
    received: Arc<Mutex<Vec<Received>>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unused_self,
    reason = "a fake server takes the real interface's arguments, used or not"
)]
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl FakeNotifications {
    #[allow(clippy::too_many_arguments, reason = "the specification's signature")]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let desktop_entry = hints
            .get("desktop-entry")
            .and_then(|value| String::try_from(value.clone()).ok());
        let mut received = self.received.lock().expect("lock");
        received.push(Received {
            app_name,
            replaces_id,
            app_icon,
            body,
            expire_timeout,
            summary,
            actions,
            desktop_entry,
        });
        u32::try_from(received.len()).expect("few notifications")
    }

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn notifications_carry_the_desktop_entry_and_report_buttons() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let server = bus.connect().await;
    let fake = FakeNotifications::default();
    let received = Arc::clone(&fake.received);
    server
        .object_server()
        .at("/org/freedesktop/Notifications", fake)
        .await
        .expect("served");
    server
        .request_name("org.freedesktop.Notifications")
        .await
        .expect("name");

    let notifier = DesktopNotifier::new(bus.connect().await);
    let mut presses = notifier.actions();
    let id = notifier
        .notify(&Notification {
            summary: "Couldn't open Firefox".into(),
            actions: vec![("open:0".into(), "Open in Chromium".into())],
            ..Notification::default()
        })
        .await
        .expect("shown");
    let seen = received.lock().expect("lock").clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].summary, "Couldn't open Firefox");
    assert_eq!(seen[0].app_name, "Wye");
    assert_eq!(seen[0].app_icon, "dev.soldunov.wye");
    assert_eq!((seen[0].replaces_id, seen[0].expire_timeout), (0, -1));
    assert!(seen[0].body.is_empty());
    assert_eq!(seen[0].actions, ["open:0", "Open in Chromium"]);
    assert_eq!(seen[0].desktop_entry.as_deref(), Some("dev.soldunov.wye"));

    let emitter = SignalEmitter::new(&server, "/org/freedesktop/Notifications").expect("emitter");
    FakeNotifications::action_invoked(&emitter, id, "open:0")
        .await
        .expect("emitted");
    let press = tokio::time::timeout(WAIT, presses.recv())
        .await
        .expect("in time")
        .expect("a press");
    assert_eq!((press.id, press.action.as_str()), (id, "open:0"));
}

struct FakeScreenSaver {
    active: bool,
}

#[zbus::interface(name = "org.freedesktop.ScreenSaver")]
impl FakeScreenSaver {
    fn get_active(&self) -> bool {
        self.active
    }

    #[zbus(signal)]
    async fn active_changed(emitter: &SignalEmitter<'_>, active: bool) -> zbus::Result<()>;
}

#[derive(Default)]
struct FakeLoginManager {
    asked: Arc<Mutex<Vec<u32>>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unused_self,
    reason = "a fake server takes the real interface's arguments, used or not"
)]
#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl FakeLoginManager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> OwnedObjectPath {
        self.asked.lock().expect("lock").push(pid);
        OwnedObjectPath::try_from("/org/freedesktop/login1/session/_31").expect("path")
    }
}

struct FakeSession {
    locked: bool,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl FakeSession {
    #[zbus(property)]
    fn locked_hint(&self) -> bool {
        self.locked
    }
}

async fn changed_to(monitor: &SessionLockMonitor, locked: bool) {
    let mut state = monitor.locked();
    tokio::time::timeout(WAIT, state.wait_for(|now| *now == locked))
        .await
        .expect("in time")
        .expect("monitor alive");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_lock_state_follows_the_screen_saver_and_logind() {
    let (Some(session_bus), Some(system_bus)) = (PrivateBus::start(), PrivateBus::start()) else {
        return;
    };
    let saver = session_bus.connect().await;
    saver
        .object_server()
        .at("/ScreenSaver", FakeScreenSaver { active: false })
        .await
        .expect("served");
    saver
        .request_name("org.freedesktop.ScreenSaver")
        .await
        .expect("name");
    let logind = system_bus.connect().await;
    let manager = FakeLoginManager::default();
    let asked = Arc::clone(&manager.asked);
    logind
        .object_server()
        .at("/org/freedesktop/login1", manager)
        .await
        .expect("served");
    logind
        .object_server()
        .at(
            "/org/freedesktop/login1/session/_31",
            FakeSession { locked: false },
        )
        .await
        .expect("served");
    logind
        .request_name("org.freedesktop.login1")
        .await
        .expect("name");

    let session = session_bus.connect().await;
    let system = system_bus.connect().await;
    let monitor = SessionLockMonitor::start(&session, Some(&system)).await;
    assert_eq!(monitor.mechanism(), Some("logind+screensaver"));
    assert_eq!(*asked.lock().expect("lock"), [std::process::id()]);
    assert!(!*monitor.locked().borrow());

    // The screen saver locks.
    let emitter = SignalEmitter::new(&saver, "/ScreenSaver").expect("emitter");
    FakeScreenSaver::active_changed(&emitter, true)
        .await
        .expect("emitted");
    changed_to(&monitor, true).await;
    FakeScreenSaver::active_changed(&emitter, false)
        .await
        .expect("emitted");
    changed_to(&monitor, false).await;

    // logind says locked.
    let session_object = logind
        .object_server()
        .interface::<_, FakeSession>("/org/freedesktop/login1/session/_31")
        .await
        .expect("served");
    session_object.get_mut().await.locked = true;
    session_object
        .get()
        .await
        .locked_hint_changed(session_object.signal_emitter())
        .await
        .expect("emitted");
    changed_to(&monitor, true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_sources_the_screen_is_never_locked() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let monitor = SessionLockMonitor::start(&bus.connect().await, None).await;
    assert_eq!(monitor.mechanism(), None);
    assert!(!*monitor.locked().borrow());
}

/// Unit name, job mode, PIDs and how many auxiliary units were asked for.
type Started = Arc<Mutex<Vec<(String, String, Vec<u32>, usize)>>>;

#[derive(Default)]
struct FakeSystemd {
    started: Started,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unused_self,
    reason = "a fake server takes the real interface's arguments, used or not"
)]
#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl FakeSystemd {
    #[allow(clippy::type_complexity, reason = "systemd's a(sa(sv))")]
    fn start_transient_unit(
        &self,
        name: String,
        mode: String,
        properties: Vec<(String, OwnedValue)>,
        aux: Vec<(String, Vec<(String, OwnedValue)>)>,
    ) -> OwnedObjectPath {
        let pids = properties
            .into_iter()
            .find(|(key, _)| key == "PIDs")
            .and_then(|(_, value)| Vec::<u32>::try_from(value).ok())
            .unwrap_or_default();
        self.started
            .lock()
            .expect("lock")
            .push((name, mode, pids, aux.len()));
        OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/1").expect("path")
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn launched_apps_get_a_transient_scope() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let server = bus.connect().await;
    let fake = FakeSystemd::default();
    let started = Arc::clone(&fake.started);
    server
        .object_server()
        .at("/org/freedesktop/systemd1", fake)
        .await
        .expect("served");
    server
        .request_name("org.freedesktop.systemd1")
        .await
        .expect("name");

    let scopes = SystemdScopes::new(bus.connect().await);
    let command = LaunchCommand {
        argv: vec!["fake-one".into()],
        desktop_id: Some("fake-one.desktop".into()),
        name: "Fake One".into(),
        ..LaunchCommand::default()
    };
    scopes.adopt(4242, &command).await.expect("scoped");
    let started = started.lock().expect("lock").clone();
    assert_eq!(started.len(), 1);
    let (name, mode, pids, aux) = &started[0];
    assert!(name.starts_with("app-wye-fake\\x2done-"), "{name}");
    assert_eq!(mode, "fail");
    assert_eq!(pids, &[4242]);
    assert_eq!(*aux, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_systemd_a_scope_fails_softly() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let scopes = SystemdScopes::new(bus.connect().await);
    let command = LaunchCommand {
        argv: vec!["fake-one".into()],
        ..LaunchCommand::default()
    };
    assert!(scopes.adopt(1, &command).await.is_err());
}
