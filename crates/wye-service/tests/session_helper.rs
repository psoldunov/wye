//! The GNOME Shell extension as the session helper (`SessionHelper1`): a
//! fake extension on a private bus answers the service's clipboard,
//! modifier, pointer and focus queries, and sends clipboard changes only
//! while the service asked for them (EXT-12). Skips without `dbus-daemon`.

mod support;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use support::{PrivateBus, eventually};
use wye_api::context::Modifier;
use wye_api::names::{GNOME_BUS_NAME, GNOME_OBJECT_PATH, SESSION_HELPER_INTERFACE};
use wye_api::picker::Placement;
use wye_service::platform::gnome_shell::{MECHANISM, ShellHelper};
use wye_service::platform::{
    ClipboardProvider as _, FocusSource as _, FocusedApp, ModifierSource as _, PlatformError,
    PointerSource as _,
};

/// What the fake extension was asked.
#[derive(Debug, Default)]
struct Log {
    watch: Vec<bool>,
    written: Vec<String>,
}

/// `SessionHelper1` as the extension serves it.
struct FakeHelper {
    clipboard: String,
    pointer: (i32, i32, String),
    modifiers: Vec<String>,
    focused: String,
    log: Arc<Mutex<Log>>,
}

#[zbus::interface(name = "dev.soldunov.wye.SessionHelper1")]
impl FakeHelper {
    fn query_pointer(&self) -> (i32, i32, String) {
        self.pointer.clone()
    }

    fn query_modifiers(&self) -> Vec<String> {
        self.modifiers.clone()
    }

    fn focused_app(&self) -> String {
        self.focused.clone()
    }

    fn read_clipboard(&self) -> String {
        self.clipboard.clone()
    }

    fn write_clipboard(&self, text: String) {
        self.lock().written.push(text);
    }

    fn watch_clipboard(&self, watch: bool) {
        self.lock().watch.push(watch);
    }
}

impl FakeHelper {
    fn lock(&self) -> std::sync::MutexGuard<'_, Log> {
        self.log.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A fake extension serving the helper, or only the name.
async fn start_extension(
    bus: &PrivateBus,
    clipboard: &str,
    serves: bool,
) -> (zbus::Connection, Arc<Mutex<Log>>) {
    let connection = bus.connect().await;
    let log = Arc::new(Mutex::new(Log::default()));
    if serves {
        let fake = FakeHelper {
            clipboard: clipboard.to_owned(),
            pointer: (640, 400, "Meta-0".to_owned()),
            modifiers: vec!["Ctrl".to_owned(), "Shift".to_owned()],
            focused: "org.gnome.Ptyxis.desktop".to_owned(),
            log: Arc::clone(&log),
        };
        connection
            .object_server()
            .at(GNOME_OBJECT_PATH, fake)
            .await
            .expect("served");
    }
    connection
        .request_name(GNOME_BUS_NAME)
        .await
        .expect("named");
    (connection, log)
}

/// The extension's unicast change signal, as `Gio.DBusConnection.emit_signal`
/// sends it with a destination.
async fn copied(extension: &zbus::Connection, to: &zbus::Connection, text: &str) {
    let destination = to.unique_name().expect("a unique name").to_owned();
    extension
        .emit_signal(
            Some(destination),
            GNOME_OBJECT_PATH,
            SESSION_HELPER_INTERFACE,
            "ClipboardChanged",
            &(text,),
        )
        .await
        .expect("sent");
}

/// Check that `happened` stays false for a while. A slow helper can only
/// make this pass, never fail, so it needs no settling time.
async fn never(what: &str, mut happened: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        assert!(!happened(), "unexpected: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn watches(log: &Arc<Mutex<Log>>) -> Vec<bool> {
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .watch
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_extension_answers_every_query_while_it_runs() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let service = bus.connect().await;
    let helper = ShellHelper::start(&service);
    assert!(
        !helper.present(),
        "nothing to ask before the extension runs"
    );
    assert!(matches!(
        helper.read().await,
        Err(PlatformError::Unavailable(_))
    ));
    assert_eq!(helper.capabilities().read, None);

    let (extension, log) = start_extension(&bus, "  https://example.com/a \n", true).await;
    eventually("the helper finds the extension", || async {
        helper.present()
    })
    .await;
    assert_eq!(helper.capabilities().read, Some(MECHANISM));
    assert_eq!(helper.capabilities().watch, Some(MECHANISM));
    assert_eq!(
        helper.read().await.expect("read"),
        Some("https://example.com/a".to_owned())
    );
    helper
        .write("https://example.com/b")
        .await
        .expect("written");
    assert_eq!(
        log.lock().unwrap_or_else(PoisonError::into_inner).written,
        vec!["https://example.com/b".to_owned()]
    );
    assert_eq!(
        helper.held().await,
        Some(vec![Modifier::Ctrl, Modifier::Shift])
    );
    assert_eq!(
        helper.pointer().await,
        Some(Placement {
            output: "Meta-0".to_owned(),
            x: 640,
            y: 400,
        })
    );
    assert_eq!(
        helper.focused().await,
        Some(FocusedApp {
            desktop_id: Some("org.gnome.Ptyxis.desktop".to_owned()),
            ..FocusedApp::default()
        })
    );

    extension
        .release_name(GNOME_BUS_NAME)
        .await
        .expect("released");
    eventually("the helper notices the extension left", || async {
        !helper.present()
    })
    .await;
    assert_eq!(helper.held().await, None);
    assert_eq!(helper.capabilities().write, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext12_clipboard_changes_come_only_while_a_rewrite_wants_them() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let service = bus.connect().await;
    let helper = ShellHelper::start(&service);
    let mut changes = helper.watch().expect("the helper can watch");
    let (extension, log) = start_extension(&bus, "", true).await;
    eventually("the helper finds the extension", || async {
        helper.present()
    })
    .await;
    eventually("the helper says it does not watch yet", || async {
        watches(&log) == [false]
    })
    .await;

    helper.set_watching(true);
    eventually("the extension is asked to watch", || async {
        watches(&log) == [false, true]
    })
    .await;
    let impostor = bus.connect().await;
    copied(&impostor, &service, "https://example.com/impostor").await;
    copied(&extension, &service, "two\nlines").await;
    copied(&extension, &service, "https://example.com/kept").await;
    let change = tokio::time::timeout(Duration::from_secs(2), changes.recv())
        .await
        .expect("a change in time")
        .expect("a change");
    assert_eq!(
        change, "https://example.com/kept",
        "only single lines, only from the extension, only while watching"
    );

    helper.set_watching(false);
    eventually("the extension is told to stop", || async {
        watches(&log) == [false, true, false]
    })
    .await;
    // A change the extension sends all the same is dropped.
    copied(&extension, &service, "https://example.com/ignored").await;
    never("a change while not watching", || {
        matches!(
            changes.try_recv(),
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
        )
    })
    .await;

    // A restarted Shell is told again what the service wants.
    helper.set_watching(true);
    extension
        .release_name(GNOME_BUS_NAME)
        .await
        .expect("released");
    eventually("gone", || async { !helper.present() }).await;
    let (_again, log) = start_extension(&bus, "", true).await;
    eventually("the new extension is asked to watch", || async {
        watches(&log) == [true]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_extension_without_the_helper_is_not_used() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    let service = bus.connect().await;
    let helper = ShellHelper::start(&service);
    let (_extension, _log) = start_extension(&bus, "", false).await;
    never("the helper to use an extension without it", || {
        helper.present()
    })
    .await;
    assert_eq!(helper.capabilities().read, None);
}
