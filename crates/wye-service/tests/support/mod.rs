//! Test fixtures: a private session bus (`dbus-daemon --session --nofork
//! --print-address=1`, killed when dropped), a throwaway desktop with two
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
}

impl PrivateBus {
    /// Start a bus, or `None` when `dbus-daemon` is not on `PATH` (the test
    /// then skips and says so).
    pub fn start() -> Option<Self> {
        let Some(program) = find_on_path(DAEMON) else {
            eprintln!("skipping: {DAEMON} is not on PATH");
            return None;
        };
        let mut daemon = Command::new(program)
            .args(["--session", "--nofork", "--print-address=1"])
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
        Some(Self { daemon, address })
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
        let bus = PrivateBus::start()?;
        let desktop = Desktop::new();
        desktop.config(config);
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
