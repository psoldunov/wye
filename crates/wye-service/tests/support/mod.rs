//! Test fixtures: a private session bus (`dbus-daemon` with a generated
//! config that has no `servicedir`, killed when dropped), a throwaway desktop with two
//! fake browsers, and the service running on both with fake platform
//! integrations.

#![allow(
    dead_code,
    reason = "each test file uses the part of the fixtures it needs"
)]

use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;
use wye_service::ServiceContext;
use wye_service::platform::fake::FakePlatform;
use wye_service::run::{self, Environment};

pub mod hosts;

/// The daemon's program name.
const DAEMON: &str = "dbus-daemon";

/// Desktop IDs of the fake browsers.
pub const ONE: &str = "fake-one.desktop";
pub const TWO: &str = "fake-two.desktop";

/// How long [`eventually`] waits.
const PATIENCE: Duration = Duration::from_secs(5);

/// A running private bus.
pub struct PrivateBus {
    daemon: Child,
    address: String,
    /// Holds the socket and the config; removed after the daemon stops.
    _dir: TempDir,
}

impl PrivateBus {
    /// Start a bus, or `None` when `dbus-daemon` is not on `PATH` (the test
    /// then skips and says so).
    pub fn start() -> Option<Self> {
        let Some(program) = find_on_path(DAEMON) else {
            eprintln!("skipping: {DAEMON} is not on PATH");
            return None;
        };
        let dir = tempfile::tempdir().expect("temp dir");
        let config = dir.path().join("bus.conf");
        fs::write(&config, bus_config(&dir.path().join("bus"))).expect("bus config");
        let mut daemon = Command::new(program)
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("dbus-daemon starts");
        let stdout = daemon.stdout.take().expect("stdout is piped");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("dbus-daemon prints its address");
        let address = address.trim().to_owned();
        assert!(!address.is_empty(), "dbus-daemon printed no address");
        Some(Self {
            daemon,
            address,
            _dir: dir,
        })
    }

    /// A new connection to this bus.
    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .expect("the address parses")
            .build()
            .await
            .expect("connected to the private bus")
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        // Already gone is fine; a leaked daemon is what this prevents.
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// A session bus config with no `servicedir`. `--session` would read the
/// system config, which on a machine with Wye, portals or a secret service
/// installed lists their service files, so the bus would start them and
/// they would outlive it.
fn bus_config(socket: &Path) -> String {
    format!(
        "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\"\n \
         \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
         <busconfig>\n  <type>session</type>\n  <listen>unix:path={}</listen>\n  \
         <policy context=\"default\">\n    \
         <allow send_destination=\"*\" eavesdrop=\"true\"/>\n    <allow eavesdrop=\"true\"/>\n    \
         <allow own=\"*\"/>\n  </policy>\n</busconfig>\n",
        socket.display()
    )
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Temporary XDG directories with two fake browsers, "Fake One" and "Fake
/// Two", and an empty `/proc`.
pub struct Desktop {
    dir: TempDir,
}

impl Desktop {
    pub fn new() -> Self {
        let desktop = Self {
            dir: tempfile::tempdir().expect("temp dir"),
        };
        for sub in [
            "home",
            "config",
            "data/applications",
            "sysdata",
            "state",
            "proc",
        ] {
            fs::create_dir_all(desktop.path(sub)).expect("dir");
        }
        desktop.app(ONE, "Fake One", "fake-one %u", true);
        desktop.app(TWO, "Fake Two", "fake-two %u", true);
        desktop
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    /// Install a desktop entry; `web` makes it a browser.
    pub fn app(&self, id: &str, name: &str, exec: &str, web: bool) {
        let mime = if web {
            "MimeType=x-scheme-handler/http;x-scheme-handler/https;\n"
        } else {
            ""
        };
        self.write(
            &format!("data/applications/{id}"),
            &format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n{mime}"),
        );
    }

    pub fn config(&self, text: &str) {
        self.write("config/wye/config.toml", text);
    }

    /// A fake process in this desktop's `/proc`.
    pub fn process(&self, pid: u32, comm: &str, parent: u32, cgroup: &str) {
        let dir = format!("proc/{pid}");
        self.write(&format!("{dir}/comm"), &format!("{comm}\n"));
        self.write(
            &format!("{dir}/status"),
            &format!("Name:\t{comm}\nPPid:\t{parent}\n"),
        );
        self.write(
            &format!("{dir}/stat"),
            &format!("{pid} ({comm}) S {parent} 0 0\n"),
        );
        self.write(&format!("{dir}/cgroup"), &format!("0::{cgroup}\n"));
        self.write(&format!("{dir}/environ"), "");
    }

    pub fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, text).expect("written");
    }

    /// The service's view of this desktop.
    pub fn environment(&self) -> Environment {
        let root: &Path = self.dir.path();
        let vars = [
            ("HOME", root.join("home")),
            ("XDG_CONFIG_HOME", root.join("config")),
            ("XDG_DATA_HOME", root.join("data")),
            ("XDG_DATA_DIRS", root.join("sysdata")),
            ("XDG_CONFIG_DIRS", root.join("sysconfig")),
            ("XDG_STATE_HOME", root.join("state")),
            ("XDG_CURRENT_DESKTOP", PathBuf::from("Test")),
            ("PATH", root.join("bin")),
        ];
        let lookup = |name: &str| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        };
        Environment {
            proc_root: root.join("proc"),
            ..Environment::from_lookup(lookup).expect("a home directory")
        }
    }
}

/// The service on a private bus with fake integrations, and a client.
pub struct Service {
    pub ctx: ServiceContext,
    pub fakes: FakePlatform,
    pub client: zbus::Connection,
    pub desktop: Desktop,
    // Dropped last: the connections need the daemon.
    pub bus: PrivateBus,
}

impl Service {
    /// Start one with `config` as `config.toml`, or `None` without
    /// `dbus-daemon`.
    pub async fn start(config: &str) -> Option<Self> {
        Self::start_with(config, |_| {}).await
    }

    /// Every launched command line, oldest first.
    pub fn launched(&self) -> Vec<Vec<String>> {
        self.fakes
            .launcher
            .launched()
            .into_iter()
            .map(|command| command.argv)
            .collect()
    }
}

/// Wait until `check` holds, up to five seconds.
pub async fn eventually<F: Future<Output = bool>>(what: &str, check: impl Fn() -> F) {
    let deadline = tokio::time::Instant::now() + PATIENCE;
    while tokio::time::Instant::now() < deadline {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {what}");
}

// Configuration, state, inventory and history tests (U07).

/// Wye's own desktop ID.
pub const WYE: &str = "dev.soldunov.wye.desktop";

impl Desktop {
    /// The service's view of this desktop on `desktop`
    /// (`XDG_CURRENT_DESKTOP`), for example `KDE`.
    pub fn environment_on(&self, desktop: &str) -> Environment {
        let environment = self.environment();
        Environment {
            xdg: wye_desktop::XdgDirs {
                current_desktops: vec![desktop.to_owned()],
                ..environment.xdg.clone()
            },
            ..environment
        }
    }

    /// Install Wye's own desktop entry, which `MakeDefault` needs.
    pub fn install_wye(&self) {
        self.app(WYE, "Wye", "wye open %u", true);
    }

    /// A file's text, or empty when it is missing.
    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap_or_default()
    }

    /// Replace a file the way editors do: write a temporary file next to it
    /// and rename it over.
    pub fn replace(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        let temp = path.with_extension("tmp-save");
        fs::write(&temp, text).expect("written");
        fs::rename(&temp, &path).expect("renamed");
    }
}

impl Service {
    /// A `dev.soldunov.wye1` proxy for the client.
    pub async fn wye(&self) -> wye_api::proxy::Wye1Proxy<'static> {
        wye_api::proxy::Wye1Proxy::new(&self.client)
            .await
            .expect("proxy")
    }

    /// The `Status` property, decoded.
    pub async fn status(&self) -> wye_api::status::Status {
        let text = self.wye().await.status().await.expect("Status");
        wye_api::json::decode("Status", &text).expect("Status JSON")
    }
}

// Files a test needs before the service starts (history race fix, U13).

impl Service {
    /// Like [`Service::start`], with `prepare` run on the desktop first:
    /// for files the service reads once, such as the history, which a
    /// startup task may load before a test could write it.
    pub async fn start_with(config: &str, prepare: impl FnOnce(&Desktop)) -> Option<Self> {
        let bus = PrivateBus::start()?;
        let desktop = Desktop::new();
        desktop.config(config);
        prepare(&desktop);
        let fakes = FakePlatform::new();
        let ctx = ServiceContext::new(fakes.platform());
        run::use_environment(&ctx, desktop.environment());
        let connection = bus.connect().await;
        run::start(&connection, &ctx)
            .await
            .expect("service started");
        let client = bus.connect().await;
        Some(Self {
            ctx,
            fakes,
            client,
            desktop,
            bus,
        })
    }
}
