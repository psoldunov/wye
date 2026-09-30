//! Global shortcuts through the `GlobalShortcuts` portal (KEY-40, ADV-05 to
//! ADV-07) against a fake portal on a private bus: it owns
//! `org.freedesktop.portal.Desktop`, answers requests the way
//! xdg-desktop-portal does (a `Request.Response` signal on the request's
//! path) and records what Wye asked for. Skips without `dbus-daemon`.

#![allow(
    clippy::used_underscore_binding,
    reason = "the fakes ignore some arguments; zbus's generated dispatch still passes them"
)]

mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::{PrivateBus, eventually};
use wye_api::shortcuts::{CLIPBOARD_ALTERNATIVE, CLIPBOARD_PRIMARY, TOGGLE_MENU};
use wye_service::platform::shortcuts::{PortalShortcuts, Preferred, portal::APP_ID};
use wye_service::platform::{PlatformError, ShortcutProvider as _};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const DESKTOP: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";

/// The shortcuts of one `BindShortcuts`: (id, preferred trigger).
type Wanted = Vec<(String, Option<String>)>;

/// What the fake portal saw.
#[derive(Debug, Default)]
struct Seen {
    registered: Vec<String>,
    sessions: Vec<String>,
    closed: Vec<String>,
    /// Each `BindShortcuts`: session, then (id, preferred trigger).
    binds: Vec<(String, Wanted)>,
    configured: usize,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Seen>>);

impl Fake {
    fn seen<T>(&self, read: impl FnOnce(&Seen) -> T) -> T {
        read(&self.0.lock().expect("not poisoned"))
    }

    fn record(&self, write: impl FnOnce(&mut Seen)) {
        write(&mut self.0.lock().expect("not poisoned"));
    }

    /// The last bind's shortcuts as the portal reports them: the preferred
    /// trigger becomes the trigger.
    fn reported(&self) -> Vec<(String, HashMap<String, OwnedValue>)> {
        self.seen(|seen| {
            seen.binds
                .last()
                .map(|(_, shortcuts)| shortcuts.clone())
                .unwrap_or_default()
        })
        .into_iter()
        .map(|(id, trigger)| {
            let info = HashMap::from([
                ("description".to_owned(), owned(&Value::from(id.clone()))),
                (
                    "trigger_description".to_owned(),
                    owned(&Value::from(trigger.unwrap_or_default())),
                ),
            ]);
            (id, info)
        })
        .collect()
    }
}

fn owned(value: &Value<'_>) -> OwnedValue {
    value.try_to_owned().expect("owned")
}

fn text(options: &HashMap<String, OwnedValue>, key: &str) -> String {
    options
        .get(key)
        .and_then(|value| String::try_from(value.try_clone().ok()?).ok())
        .unwrap_or_default()
}

/// The sender as it appears in request and session paths.
fn sender_id(header: &Header<'_>) -> String {
    header
        .sender()
        .map(|sender| sender.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default()
}

fn request_path(header: &Header<'_>, options: &HashMap<String, OwnedValue>) -> OwnedObjectPath {
    let path = format!(
        "{PATH}/request/{}/{}",
        sender_id(header),
        text(options, "handle_token")
    );
    OwnedObjectPath::try_from(path).expect("request path")
}

/// Answer the request at `path` once the method has returned, the way the
/// real portal does.
fn respond(
    connection: &zbus::Connection,
    header: &Header<'_>,
    path: &OwnedObjectPath,
    results: HashMap<String, OwnedValue>,
) {
    let connection = connection.clone();
    let sender = header.sender().map(zbus::names::UniqueName::to_owned);
    let path = path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        connection
            .emit_signal(
                sender,
                path,
                "org.freedesktop.portal.Request",
                "Response",
                &(0_u32, results),
            )
            .await
            .expect("response sent");
    });
}

struct Shortcuts(Fake);

#[allow(
    clippy::needless_pass_by_value,
    reason = "a fake server takes the real interface's arguments, used or not"
)]
#[zbus::interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl Shortcuts {
    #[zbus(property, name = "version")]
    #[allow(clippy::unused_self, reason = "a property getter takes self")]
    fn version(&self) -> u32 {
        2
    }

    async fn create_session(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let session = format!(
            "{PATH}/session/{}/{}",
            sender_id(&header),
            text(&options, "session_handle_token")
        );
        connection
            .object_server()
            .at(session.as_str(), Session(self.0.clone(), session.clone()))
            .await
            .expect("session served");
        self.0.record(|seen| seen.sessions.push(session.clone()));
        let request = request_path(&header, &options);
        let results = HashMap::from([("session_handle".to_owned(), owned(&Value::from(session)))]);
        respond(connection, &header, &request, results);
        request
    }

    fn bind_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        session: OwnedObjectPath,
        shortcuts: Vec<(String, HashMap<String, OwnedValue>)>,
        _parent_window: String,
        options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let wanted = shortcuts
            .iter()
            .map(|(id, info)| {
                let preferred = Some(text(info, "preferred_trigger")).filter(|t| !t.is_empty());
                (id.clone(), preferred)
            })
            .collect();
        self.0
            .record(|seen| seen.binds.push((session.to_string(), wanted)));
        let request = request_path(&header, &options);
        let results = HashMap::from([(
            "shortcuts".to_owned(),
            owned(&Value::from(self.0.reported())),
        )]);
        respond(connection, &header, &request, results);
        request
    }

    fn list_shortcuts(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        _session: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> OwnedObjectPath {
        let request = request_path(&header, &options);
        let results = HashMap::from([(
            "shortcuts".to_owned(),
            owned(&Value::from(self.0.reported())),
        )]);
        respond(connection, &header, &request, results);
        request
    }

    fn configure_shortcuts(
        &self,
        _session: OwnedObjectPath,
        _parent_window: String,
        _options: HashMap<String, OwnedValue>,
    ) {
        self.0.record(|seen| seen.configured += 1);
    }

    #[zbus(signal)]
    async fn activated(
        emitter: &SignalEmitter<'_>,
        session: ObjectPath<'_>,
        shortcut_id: &str,
        timestamp: u64,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<()>;
}

struct Registry(Fake);

#[allow(
    clippy::needless_pass_by_value,
    reason = "a fake server takes the real interface's arguments, used or not"
)]
#[zbus::interface(name = "org.freedesktop.host.portal.Registry")]
impl Registry {
    #[zbus(property, name = "version")]
    #[allow(clippy::unused_self, reason = "a property getter takes self")]
    fn version(&self) -> u32 {
        1
    }

    fn register(&self, app_id: &str, _options: HashMap<String, OwnedValue>) {
        self.0
            .record(|seen| seen.registered.push(app_id.to_owned()));
    }
}

struct Session(Fake, String);

#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl Session {
    #[zbus(property, name = "version")]
    #[allow(clippy::unused_self, reason = "a property getter takes self")]
    fn version(&self) -> u32 {
        1
    }

    fn close(&self) {
        let path = self.1.clone();
        self.0.record(|seen| seen.closed.push(path));
    }
}

/// The fake portal on `bus`, and the connection that owns it.
async fn fake_portal(bus: &PrivateBus) -> (Fake, zbus::Connection) {
    let fake = Fake::default();
    let connection = bus.connect().await;
    let server = connection.object_server();
    server
        .at(PATH, Shortcuts(fake.clone()))
        .await
        .expect("served");
    server
        .at(PATH, Registry(fake.clone()))
        .await
        .expect("served");
    connection.request_name(DESKTOP).await.expect("name");
    (fake, connection)
}

async fn start(bus: &PrivateBus, preferred: Preferred) -> PortalShortcuts {
    PortalShortcuts::start(bus.connect().await, preferred)
        .await
        .expect("portal session")
}

async fn bound_count(fake: &Fake, count: usize) {
    eventually("the shortcuts to be bound", || async {
        fake.seen(|seen| seen.binds.len()) >= count
    })
    .await;
}

#[tokio::test]
async fn registers_wye_and_binds_every_action_with_its_preference_key_40() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (fake, _portal) = fake_portal(&bus).await;
    let portal = start(
        &bus,
        Preferred::from([(TOGGLE_MENU, "Ctrl+Alt+w".to_owned())]),
    )
    .await;
    bound_count(&fake, 1).await;

    assert_eq!(fake.seen(|seen| seen.registered.clone()), [APP_ID]);
    let (session, wanted) = fake.seen(|seen| seen.binds[0].clone());
    assert_eq!(fake.seen(|seen| seen.sessions.clone()), [session]);
    assert_eq!(
        wanted,
        [
            (TOGGLE_MENU.to_owned(), Some("CTRL+ALT+w".to_owned())),
            (CLIPBOARD_PRIMARY.to_owned(), None),
            (CLIPBOARD_ALTERNATIVE.to_owned(), None),
        ]
    );
    // The portal's answer lands after it recorded the bind.
    eventually("the portal's report", || async {
        !portal.bindings().await.expect("listed").is_empty()
    })
    .await;
    let bindings = portal.bindings().await.expect("listed");
    let toggle = bindings
        .iter()
        .find(|shortcut| shortcut.action == TOGGLE_MENU)
        .expect("toggle-menu reported");
    assert_eq!(toggle.trigger.as_deref(), Some("CTRL+ALT+w"));
    let primary = bindings
        .iter()
        .find(|shortcut| shortcut.action == CLIPBOARD_PRIMARY)
        .expect("clipboard-primary reported");
    assert_eq!(primary.trigger, None, "an empty description is unbound");
    assert_eq!(portal.mechanism(), Some("portal"));
}

#[tokio::test]
async fn a_press_of_a_wye_shortcut_is_forwarded_adv_05() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (fake, connection) = fake_portal(&bus).await;
    let portal = start(&bus, Preferred::new()).await;
    bound_count(&fake, 1).await;
    let mut presses = portal.activations();
    let session = fake.seen(|seen| seen.sessions[0].clone());
    let emitter = SignalEmitter::new(&connection, PATH).expect("emitter");
    for id in ["something-else", CLIPBOARD_ALTERNATIVE] {
        Shortcuts::activated(
            &emitter,
            ObjectPath::try_from(session.as_str()).expect("path"),
            id,
            0,
            HashMap::new(),
        )
        .await
        .expect("sent");
    }
    let forwarded = tokio::time::timeout(Duration::from_secs(5), presses.recv())
        .await
        .expect("a press arrived")
        .expect("received");
    assert_eq!(forwarded, CLIPBOARD_ALTERNATIVE, "unknown IDs are dropped");
}

#[tokio::test]
async fn binding_again_opens_a_new_session_adv_07() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (fake, _portal) = fake_portal(&bus).await;
    let portal = start(
        &bus,
        Preferred::from([(TOGGLE_MENU, "Ctrl+Alt+w".to_owned())]),
    )
    .await;
    bound_count(&fake, 1).await;

    portal
        .bind(CLIPBOARD_ALTERNATIVE, "Super+v")
        .await
        .expect("bound");
    bound_count(&fake, 2).await;
    let (first, second) = fake.seen(|seen| (seen.sessions[0].clone(), seen.binds[1].clone()));
    assert_eq!(
        fake.seen(|seen| seen.closed.clone()),
        std::slice::from_ref(&first)
    );
    assert_ne!(second.0, first, "the second bind uses a new session");
    assert_eq!(
        second.1,
        [
            (TOGGLE_MENU.to_owned(), Some("CTRL+ALT+w".to_owned())),
            (CLIPBOARD_PRIMARY.to_owned(), None),
            (CLIPBOARD_ALTERNATIVE.to_owned(), Some("LOGO+v".to_owned())),
        ]
    );

    portal.bind(TOGGLE_MENU, "").await.expect("cleared");
    bound_count(&fake, 3).await;
    let cleared = fake.seen(|seen| seen.binds[2].1[0].clone());
    assert_eq!(cleared, (TOGGLE_MENU.to_owned(), None));
}

#[tokio::test]
async fn change_opens_the_portals_own_dialog_key_40() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (fake, _portal) = fake_portal(&bus).await;
    let portal = start(&bus, Preferred::new()).await;
    bound_count(&fake, 1).await;
    portal.configure().await.expect("configured");
    assert_eq!(fake.seen(|seen| seen.configured), 1);
}

#[tokio::test]
async fn an_unknown_action_is_refused() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let (_fake, _portal) = fake_portal(&bus).await;
    let portal = start(&bus, Preferred::new()).await;
    assert!(matches!(
        portal.bind("paste", "Ctrl+v").await,
        Err(PlatformError::Failed(_))
    ));
}

#[tokio::test]
async fn a_portal_without_global_shortcuts_is_unavailable_key_41() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    // Only the registry: the bus must not start a real portal for the
    // name, so the fake owns it without the shortcuts interface.
    let fake = Fake::default();
    let connection = bus.connect().await;
    connection
        .object_server()
        .at(PATH, Registry(fake.clone()))
        .await
        .expect("served");
    connection.request_name(DESKTOP).await.expect("name");
    let started = PortalShortcuts::start(bus.connect().await, Preferred::new()).await;
    assert!(
        matches!(
            started,
            Err(PlatformError::Unavailable(_) | PlatformError::Timeout(_))
        ),
        "{started:?}"
    );
}
