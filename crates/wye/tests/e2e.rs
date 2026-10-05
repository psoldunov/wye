//! Whole-system tests: a private session bus whose `servicedir` holds the
//! shipped D-Bus service file (`data/dbus/dev.soldunov.wye.service.in`, its
//! `@bindir@` filled in), the real `wye service` and `wye open` binaries, and
//! fake browsers whose executables log their arguments (DEF-04, IN-07, PIPE-13,
//! PICK-29). A fake `dev.soldunov.wye.PickerHost1` stands in for `wye-ui`.
//! Skips (and says so) when `dbus-daemon` is not on `PATH`.

#![allow(
    dead_code,
    reason = "the shared desktop fixture serves more tests than this file runs"
)]

#[path = "cli/support.rs"]
mod support;

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead as _, BufReader};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::{Desktop, ONE, TWO};
use wye_api::names::{UI_BUS_NAME, UI_OBJECT_PATH};
use wye_api::proxy::Wye1Proxy;
use zbus::zvariant::Value;

const SERVICE_NAME: &str = "dev.soldunov.wye";
const SERVICE_FILE: &str = "dev.soldunov.wye.service";
const BINDIR_PLACEHOLDER: &str = "@bindir@";
const URL: &str = "https://example.com/";
const PATIENCE: Duration = Duration::from_secs(5);

/// The session bus, activating `wye service` from the shipped template.
struct Bus {
    daemon: Child,
    address: String,
}

impl Bus {
    fn start(desktop: &Desktop) -> Option<Self> {
        Self::start_with_env(desktop, &[])
    }

    /// [`Bus::start`], with `extra` added to the daemon's environment (and so
    /// to the activated service's), replacing a variable of the same name.
    fn start_with_env(desktop: &Desktop, extra: &[(&'static str, OsString)]) -> Option<Self> {
        let program = find_on_path("dbus-daemon").or_else(|| {
            eprintln!("skipping: dbus-daemon is not on PATH");
            None
        })?;
        let services = desktop.path("dbus-services");
        fs::create_dir_all(&services).expect("servicedir");
        fs::write(services.join(SERVICE_FILE), shipped_service_file()).expect("service file");
        let socket = desktop.path("bus");
        let config = desktop.path("bus.conf");
        fs::write(&config, bus_config(&socket, &services)).expect("bus config");
        let address = format!("unix:path={}", socket.display());
        let mut daemon = Command::new(program)
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .env_clear()
            .envs(bus_env(desktop, &address, extra))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            // An activated service inherits stderr; a pipe would hold the
            // test harness open for as long as the service lives.
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon starts");
        let mut printed = String::new();
        BufReader::new(daemon.stdout.take().expect("piped"))
            .read_line(&mut printed)
            .expect("the daemon prints its address");
        assert!(!printed.trim().is_empty(), "dbus-daemon printed no address");
        Some(Self { daemon, address })
    }

    async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .expect("the address parses")
            .build()
            .await
            .expect("connected to the private bus")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        // The activated service is a child of the daemon, not of the test:
        // stop it first, or it outlives the bus.
        stop_service(&self.address);
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// The shipped template with `@bindir@` pointing at the binary under test.
/// `SystemdService=` stays: a bus not started with systemd activation ignores
/// it, and the assertion below keeps the template honest.
fn shipped_service_file() -> String {
    let template = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/dbus/dev.soldunov.wye.service.in"
    );
    let text = fs::read_to_string(template).expect("the shipped service file template");
    assert!(
        text.contains(&format!("Name={SERVICE_NAME}\n")) && text.contains(BINDIR_PLACEHOLDER),
        "unexpected template:\n{text}"
    );
    let binary = Path::new(env!("CARGO_BIN_EXE_wye"));
    let bindir = binary.parent().expect("the binary has a directory");
    // The test binary is called `wye`, like the installed one.
    assert_eq!(binary.file_name().and_then(|n| n.to_str()), Some("wye"));
    text.replace(BINDIR_PLACEHOLDER, &bindir.display().to_string())
}

fn bus_config(socket: &Path, services: &Path) -> String {
    format!(
        "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\"\n \
         \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
         <busconfig>\n  <type>session</type>\n  <listen>unix:path={}</listen>\n  \
         <servicedir>{}</servicedir>\n  <policy context=\"default\">\n    \
         <allow send_destination=\"*\" eavesdrop=\"true\"/>\n    <allow eavesdrop=\"true\"/>\n    \
         <allow own=\"*\"/>\n  </policy>\n</busconfig>\n",
        socket.display(),
        services.display()
    )
}

/// The desktop's environment with the private bus as the session bus, which
/// the activated service inherits, and `extra` on top: a variable of the
/// same name is replaced, any other is added.
fn bus_env(
    desktop: &Desktop,
    address: &str,
    extra: &[(&'static str, OsString)],
) -> Vec<(&'static str, OsString)> {
    desktop
        .env()
        .into_iter()
        .filter(|(name, _)| {
            *name != "DBUS_SESSION_BUS_ADDRESS" && extra.iter().all(|(other, _)| other != name)
        })
        .chain([("DBUS_SESSION_BUS_ADDRESS", address.into())])
        .chain(extra.iter().cloned())
        .collect()
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Kill whatever process owns the service name on the bus at `address`.
fn stop_service(address: &str) {
    let Some(dbus_send) = find_on_path("dbus-send") else {
        return;
    };
    let Ok(output) = Command::new(dbus_send)
        .args([
            &format!("--bus={address}"),
            "--print-reply",
            "--dest=org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.GetConnectionUnixProcessID",
            &format!("string:{SERVICE_NAME}"),
        ])
        .output()
    else {
        return;
    };
    let pid = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("uint32 ").map(str::to_owned));
    if let Some(pid) = pid {
        let _ = Command::new("kill").arg(pid).status();
    }
}

/// What the fake UI host was asked to show.
#[derive(Clone, Default)]
struct Shown(Arc<Mutex<Vec<(String, String)>>>);

struct PickerHost(Shown);

#[allow(
    clippy::unused_self,
    reason = "the fake serves the whole interface; members it ignores still take their arguments"
)]
#[zbus::interface(name = "dev.soldunov.wye.PickerHost1")]
impl PickerHost {
    fn show_picker(&self, request_id: &str, request: &str) {
        self.0
            .0
            .lock()
            .expect("not poisoned")
            .push((request_id.to_owned(), request.to_owned()));
    }

    fn close_picker(&self, request_id: &str) {
        let _ = request_id;
    }

    fn show_menu(&self, menu: &str) {
        let _ = menu;
    }
}

async fn wait_until<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = tokio::time::Instant::now() + PATIENCE;
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

const PRIMARY_ONE: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";
const PICKER: &str = "[browsers]\nprimary = { picker = true }\n";

#[test]
fn open_reaches_the_browser_through_the_shipped_service_file() {
    // DEF-04, IN-07, LAUNCH: link → `wye open` → activated `wye service` →
    // fake browser's argv.
    let desktop = Desktop::new();
    desktop.config(PRIMARY_ONE);
    let Some(bus) = Bus::start(&desktop) else {
        return;
    };
    let run = desktop
        .wye_on_bus(&["open", "https://example.com/?utm_source=x"], &bus.address)
        .expect_code(0);
    assert_eq!(run.stderr, "");
    assert_eq!(desktop.wait_for_log(ONE), format!("{URL}\n"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_picker_route_waits_for_the_choice_and_opens_the_chosen_browser() {
    // PIPE-13, PICK-29: `wye open` returns once the picker is up; the choice
    // arrives as `PickerChose` from the UI host and launches the browser.
    let desktop = Desktop::new();
    desktop.config(PICKER);
    let Some(bus) = Bus::start(&desktop) else {
        return;
    };
    pick(&desktop, &bus, r#"{"app":"fake-two.desktop"}"#).await;
    assert_eq!(desktop.wait_for_log(TWO), format!("{URL}\n"));
    assert!(desktop.never_launched(ONE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_private_window_choice_opens_the_browser_privately() {
    // KEY-13: the picker sends a private-capable browser's private target.
    let desktop = Desktop::new();
    desktop.add_browser("firefox.desktop", "Firefox");
    desktop.config(PICKER);
    let Some(bus) = Bus::start(&desktop) else {
        return;
    };
    pick(&desktop, &bus, r#"{"private":"firefox.desktop"}"#).await;
    let log = desktop.wait_for_log("firefox.desktop");
    assert!(log.lines().any(|arg| arg == "--private-window"), "{log}");
    assert!(log.lines().any(|arg| arg == URL), "{log}");
}

#[test]
fn a_service_in_an_appimage_launches_apps_with_the_session_environment() {
    // LAUNCH-08: the service runs from inside a fake AppDir with the
    // runtime's variables set; the app it launches sees none of them.
    let desktop = Desktop::new();
    let log = desktop.path("fake-env.log");
    let script = desktop.path("bin/fake-env");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nenv > '{0}.tmp' && mv '{0}.tmp' '{0}'\n",
            log.display()
        ),
    )
    .expect("script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("mode");
    desktop.write(
        "data/applications/fake-env.desktop",
        &format!(
            "[Desktop Entry]\nType=Application\nName=Fake Env\nExec={} %u\n\
             MimeType=x-scheme-handler/http;x-scheme-handler/https;\n",
            script.display()
        ),
    );
    desktop.config("[browsers]\nprimary = { app = \"fake-env.desktop\" }\n");

    let binary = Path::new(env!("CARGO_BIN_EXE_wye"));
    let appdir = fs::canonicalize(binary.parent().expect("a directory")).expect("canonical");
    let appdir_text = appdir.display().to_string();
    let home = desktop.path("home");
    let sysdata = desktop.path("sysdata");
    let session_path = desktop_env(&desktop, "PATH");
    let Some(bus) = Bus::start_with_env(
        &desktop,
        &[
            ("APPDIR", appdir.clone().into()),
            ("SHARUN_DIR", appdir.clone().into()),
            ("URUNTIME", "/nonexistent/Wye.AppImage".into()),
            ("PATH", format!("{appdir_text}/bin:{session_path}").into()),
            (
                "XDG_DATA_DIRS",
                format!("{appdir_text}/share:{}", sysdata.display()).into(),
            ),
            (
                "GIO_LAUNCH_DESKTOP",
                format!("{appdir_text}/bin/gio-launch-desktop").into(),
            ),
            ("XDG_CACHE_HOME", home.join(".cache/AppImage-Cache").into()),
            ("HOST_HOME", home.clone().into()),
            ("HOST_XDG_CACHE_HOME", home.join(".cache").into()),
        ],
    ) else {
        return;
    };
    desktop
        .wye_on_bus(&["open", URL], &bus.address)
        .expect_code(0);

    let dumped = desktop.wait_for_log("fake-env.desktop");
    let vars: HashMap<&str, &str> = dumped
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    for gone in [
        "APPDIR",
        "SHARUN_DIR",
        "URUNTIME",
        "HOST_HOME",
        "HOST_XDG_CACHE_HOME",
        "GIO_LAUNCH_DESKTOP",
    ] {
        assert!(!vars.contains_key(gone), "{gone} leaked:\n{dumped}");
    }
    assert_eq!(
        vars.get("XDG_CACHE_HOME").copied(),
        Some(home.join(".cache").to_str().expect("UTF-8"))
    );
    assert_eq!(
        vars.get("XDG_DATA_DIRS").copied(),
        Some(sysdata.to_str().expect("UTF-8"))
    );
    let expected_path: Vec<&str> = session_path
        .split(':')
        .filter(|entry| !Path::new(entry).starts_with(&appdir))
        .collect();
    assert_eq!(
        vars.get("PATH")
            .map(|path| path.split(':').collect::<Vec<_>>()),
        Some(expected_path)
    );
    for needle in [format!("{appdir_text}/bin"), format!("{appdir_text}/share")] {
        assert!(!dumped.contains(&needle), "{needle} leaked:\n{dumped}");
    }
}

/// The value of `name` in the desktop's environment.
fn desktop_env(desktop: &Desktop, name: &str) -> String {
    desktop
        .env()
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .and_then(|(_, value)| value.into_string().ok())
        .expect("a UTF-8 value")
}

/// Open [`URL`] with `wye open` on a picker route, answer the picker the
/// fake UI host shows with `choice`, and check `wye open` succeeded.
async fn pick(desktop: &Desktop, bus: &Bus, choice: &str) {
    let ui = bus.connect().await;
    let shown = Shown::default();
    ui.object_server()
        .at(UI_OBJECT_PATH, PickerHost(shown.clone()))
        .await
        .expect("served");
    ui.request_name(UI_BUS_NAME).await.expect("name");

    let address = bus.address.clone();
    let opener = tokio::task::spawn_blocking({
        let env = desktop.env();
        move || {
            Command::new(env!("CARGO_BIN_EXE_wye"))
                .args(["open", URL])
                .env_clear()
                .envs(env)
                .env("DBUS_SESSION_BUS_ADDRESS", address)
                .output()
                .expect("wye open runs")
        }
    });

    let (id, request) = wait_until("the picker to be shown", || {
        shown.0.lock().expect("not poisoned").first().cloned()
    })
    .await;
    assert!(request.contains(URL), "{request}");
    assert!(
        desktop.never_launched(TWO) && desktop.never_launched(ONE),
        "nothing opens before the choice"
    );

    let wye = Wye1Proxy::new(&ui).await.expect("proxy");
    wye.picker_chose(&id, choice, HashMap::<&str, Value>::new())
        .await
        .expect("chosen");
    let output = opener.await.expect("joined");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}
