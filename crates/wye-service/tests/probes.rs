//! Session probes end to end (PICK-02, source-app step 4, KEY-06).
//!
//! The query script runs in `QuickJS` against a stub `workspace`; on a
//! private bus a fake `KWin` loads it, runs it the same way and calls
//! `KWin1.Report` with what the script passed to `callDBus`, typed as
//! `KWin` types them. Skips the bus tests without `dbus-daemon`.

#[path = "probes/script.rs"]
mod script;
mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use script::Scene;
use serde_json::{Value, json};
use support::{Desktop, PrivateBus, eventually};
use wye_api::names::{KWIN_INTERFACE, OBJECT_PATH};
use wye_api::picker::Placement;
use wye_service::ServiceContext;
use wye_service::platform::fake::FakePlatform;
use wye_service::platform::kwin::script::{ReplyTo, render};
use wye_service::platform::kwin::scripting::KWinScripting;
use wye_service::platform::kwin::{KWinHelper, MECHANISM};
use wye_service::platform::{FocusSource as _, FocusedApp, Platform, PointerSource};
use wye_service::run;

/// Generous: the private bus and `QuickJS` run in the test process.
const TIMEOUT: Duration = Duration::from_secs(2);

/// For the test that waits for no answer.
const SHORT: Duration = Duration::from_millis(300);

fn reply_to(service: &str) -> ReplyTo {
    ReplyTo {
        service: service.to_owned(),
        path: OBJECT_PATH.to_owned(),
        interface: KWIN_INTERFACE.to_owned(),
    }
}

fn rendered(scene: &Scene) -> Value {
    let text = render(&reply_to(":1.42"), "0123abcd").expect("renders");
    script::run(&text, scene)
}

#[test]
fn the_script_reports_the_pointer_on_its_output_and_the_active_window() {
    assert_eq!(
        rendered(&Scene::dolphin()),
        json!([
            ":1.42",
            "/dev/soldunov/wye",
            "dev.soldunov.wye.KWin1",
            "Report",
            "0123abcd",
            109,
            1252,
            "DP-1",
            4242,
            "org.kde.dolphin",
            "dolphin"
        ])
    );
}

#[test]
fn without_an_output_or_a_window_the_script_sends_empty_values() {
    let scene = Scene {
        output: None,
        window: None,
        ..Scene::dolphin()
    };
    assert_eq!(
        rendered(&scene),
        json!([
            ":1.42",
            "/dev/soldunov/wye",
            "dev.soldunov.wye.KWin1",
            "Report",
            "0123abcd",
            2030,
            1252,
            "",
            0,
            "",
            ""
        ])
    );
}

/// What the fake `KWin` did.
#[derive(Debug, Default)]
struct Log {
    loaded: Vec<String>,
    unloaded: Vec<String>,
    reports: Vec<Result<(), String>>,
}

/// How the fake `KWin` answers a started script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Behaviour {
    /// Run it and report from its own connection, as `KWin` does.
    Answer,
    /// Run it and report from another client's connection.
    Impostor,
}

/// `org.kde.kwin.Scripting` on `/Scripting`, running scripts in `QuickJS`.
struct FakeKWin {
    scene: Scene,
    behaviour: Behaviour,
    impostor: zbus::Connection,
    scripts: Mutex<Vec<(String, PathBuf)>>,
    log: Arc<Mutex<Log>>,
}

#[zbus::interface(name = "org.kde.kwin.Scripting")]
impl FakeKWin {
    #[zbus(name = "loadScript")]
    fn load_script(&self, path: String, name: String) -> i32 {
        let mut scripts = self.scripts.lock().unwrap_or_else(PoisonError::into_inner);
        scripts.push((name.clone(), PathBuf::from(path)));
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .loaded
            .push(name);
        i32::try_from(scripts.len()).unwrap_or(i32::MAX) - 1
    }

    #[zbus(name = "start")]
    fn start(&self, #[zbus(connection)] connection: &zbus::Connection) {
        let scripts = self.scripts.lock().unwrap_or_else(PoisonError::into_inner);
        let Some((_, path)) = scripts.last() else {
            return;
        };
        let text = std::fs::read_to_string(path).expect("the script file exists while loaded");
        let args = script::run(&text, &self.scene);
        let from = match self.behaviour {
            Behaviour::Answer => connection.clone(),
            Behaviour::Impostor => self.impostor.clone(),
        };
        let log = Arc::clone(&self.log);
        // `callDBus` does not wait for the answer; neither does `start`.
        tokio::spawn(async move {
            let result = report(&from, &args)
                .await
                .map_err(|error| error.to_string());
            log.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reports
                .push(result);
        });
    }

    #[zbus(name = "unloadScript")]
    fn unload_script(&self, name: String) -> bool {
        let mut scripts = self.scripts.lock().unwrap_or_else(PoisonError::into_inner);
        let before = scripts.len();
        scripts.retain(|(loaded, _)| *loaded != name);
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unloaded
            .push(name);
        scripts.len() < before
    }
}

/// Call what the script asked for, typing numbers as `int32` like `KWin`.
async fn report(from: &zbus::Connection, args: &Value) -> zbus::Result<()> {
    let text = |index: usize| args[index].as_str().expect("a string").to_owned();
    let int = |index: usize| {
        i32::try_from(args[index].as_i64().expect("an integer")).expect("fits an int32")
    };
    let body = (text(4), int(5), int(6), text(7), int(8), text(9), text(10));
    from.call_method(
        Some(text(0).as_str()),
        text(1).as_str(),
        Some(text(2).as_str()),
        text(3).as_str(),
        &body,
    )
    .await
    .map(drop)
}

/// A service with a real `KWin` helper, and a fake `KWin`, on one bus.
struct Setup {
    helper: Arc<KWinHelper>,
    log: Arc<Mutex<Log>>,
    scripts: tempfile::TempDir,
    /// The service's files: never the real home directory.
    _desktop: Desktop,
    _service: zbus::Connection,
    _kwin: zbus::Connection,
    _bus: PrivateBus,
}

async fn setup(behaviour: Behaviour, timeout: Duration) -> Option<Setup> {
    let bus = PrivateBus::start()?;
    let kwin = bus.connect().await;
    let impostor = bus.connect().await;
    let log = Arc::new(Mutex::new(Log::default()));
    let fake = FakeKWin {
        scene: Scene::dolphin(),
        behaviour,
        impostor,
        scripts: Mutex::default(),
        log: Arc::clone(&log),
    };
    kwin.object_server()
        .at("/Scripting", fake)
        .await
        .expect("served");
    kwin.request_name("org.kde.KWin").await.expect("named");

    let service = bus.connect().await;
    let scripts = tempfile::tempdir().expect("temp dir");
    let fakes = FakePlatform::new();
    let base = fakes.platform();
    let unique = service.unique_name().expect("a unique name").to_string();
    let helper = Arc::new(
        KWinHelper::new(
            Arc::new(KWinScripting::new(service.clone())),
            base.kwin_reports.clone(),
            reply_to(&unique),
            scripts.path().join("kwin"),
        )
        .with_timeout(timeout),
    );
    let platform = Platform {
        pointer: Arc::clone(&helper) as _,
        focus: Arc::clone(&helper) as _,
        ..base
    };
    let ctx = ServiceContext::new(platform);
    let desktop = Desktop::new();
    run::use_environment(&ctx, desktop.environment());
    run::start(&service, &ctx)
        .await
        .expect("the service starts");
    Some(Setup {
        helper,
        log,
        scripts,
        _desktop: desktop,
        _service: service,
        _kwin: kwin,
        _bus: bus,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kwin_answers_pointer_and_focus_through_the_bus_pick_02() {
    let Some(setup) = setup(Behaviour::Answer, TIMEOUT).await else {
        return;
    };
    assert_eq!(
        setup.helper.pointer().await,
        Some(Placement {
            output: "DP-1".to_owned(),
            x: 109,
            y: 1252,
        })
    );
    assert_eq!(
        setup.helper.focused().await,
        Some(FocusedApp {
            pid: Some(4242),
            desktop_id: Some("org.kde.dolphin".to_owned()),
            resource_class: Some("dolphin".to_owned()),
        })
    );
    assert_eq!(
        PointerSource::mechanism(setup.helper.as_ref()),
        Some(MECHANISM)
    );

    eventually("the fake KWin's report returns", || async {
        !setup
            .log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reports
            .is_empty()
    })
    .await;
    let log = setup.log.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(log.loaded.len(), 1, "one query answers both");
    assert_eq!(log.unloaded, log.loaded, "the script is unloaded");
    assert_eq!(log.reports, vec![Ok(())]);
    let left = std::fs::read_dir(setup.scripts.path().join("kwin"))
        .expect("the script directory")
        .count();
    assert_eq!(left, 0, "the script file is removed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_report_from_another_client_is_refused_and_the_query_times_out() {
    let Some(setup) = setup(Behaviour::Impostor, SHORT).await else {
        return;
    };
    assert_eq!(setup.helper.pointer().await, None);

    eventually("the impostor's report returns", || async {
        !setup
            .log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reports
            .is_empty()
    })
    .await;
    let log = setup.log.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(log.unloaded, log.loaded, "unloaded after the timeout too");
    let [Err(refused)] = log.reports.as_slice() else {
        panic!("expected one refused report: {:?}", log.reports);
    };
    assert!(
        refused.contains("only accepted from the compositor"),
        "{refused}"
    );
}
