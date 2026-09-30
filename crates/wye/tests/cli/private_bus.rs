//! A private session bus for end-to-end tests, optionally able to start
//! `wye service` through D-Bus activation from a temporary `servicedir`.

use std::ffi::OsString;
use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::support::Desktop;

const DAEMON: &str = "dbus-daemon";
const SERVICE_NAME: &str = "dev.soldunov.wye";

/// A running `dbus-daemon`, killed with anything it started when dropped.
pub struct PrivateBus {
    daemon: Child,
    pub address: String,
}

impl PrivateBus {
    /// A bus whose activatable services include `wye service`, running with
    /// `desktop`'s environment; `None` (and a note) without `dbus-daemon`.
    pub fn with_wye_activatable(desktop: &Desktop) -> Option<Self> {
        let program = find_on_path(DAEMON)?;
        let services = desktop.path("dbus-services");
        std::fs::create_dir_all(&services).unwrap();
        std::fs::write(
            services.join(format!("{SERVICE_NAME}.service")),
            format!(
                "[D-BUS Service]\nName={SERVICE_NAME}\nExec={} service\n",
                env!("CARGO_BIN_EXE_wye")
            ),
        )
        .unwrap();
        let socket = desktop.path("bus");
        let config = desktop.path("bus.conf");
        std::fs::write(&config, config_file(&socket, &services)).unwrap();
        let address = format!("unix:path={}", socket.display());
        // Activated services inherit this environment.
        let env: Vec<(&str, OsString)> = desktop
            .env()
            .into_iter()
            .filter(|(name, _)| *name != "DBUS_SESSION_BUS_ADDRESS")
            .chain([("DBUS_SESSION_BUS_ADDRESS", OsString::from(&address))])
            .collect();
        let mut command = Command::new(program);
        command
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .env_clear()
            .envs(env);
        Some(Self::spawn(command))
    }

    /// A plain session bus.
    pub fn plain() -> Option<Self> {
        let program = find_on_path(DAEMON)?;
        let mut command = Command::new(program);
        command.args(["--session", "--nofork", "--print-address=1"]);
        Some(Self::spawn(command))
    }

    fn spawn(mut command: Command) -> Self {
        let mut daemon = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            // Activated services inherit it: a pipe the test harness reads
            // would keep the harness waiting for them.
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon starts");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().expect("piped"))
            .read_line(&mut address)
            .expect("dbus-daemon prints its address");
        Self {
            daemon,
            address: address.trim().to_owned(),
        }
    }

    /// Whether `dev.soldunov.wye` is owned on this bus, via `dbus-send`.
    pub fn service_running(&self) -> bool {
        self.name_owner_pid().is_some()
    }

    /// Wait up to five seconds for the service to own its name.
    pub fn wait_for_service(&self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.service_running() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    /// The service's process ID, when it runs.
    fn name_owner_pid(&self) -> Option<u32> {
        let output = Command::new(find_on_path("dbus-send")?)
            .args([
                &format!("--bus={}", self.address),
                "--print-reply",
                "--dest=org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.GetConnectionUnixProcessID",
                &format!("string:{SERVICE_NAME}"),
            ])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.trim().strip_prefix("uint32 "))
            .and_then(|pid| pid.trim().parse().ok())
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        // A service started on this bus would outlive it: stop it first.
        if let Some(pid) = self.name_owner_pid() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// `true` (with a note) when the tools the bus tests need are missing.
pub fn missing_tools() -> bool {
    let missing: Vec<&str> = [DAEMON, "dbus-send"]
        .into_iter()
        .filter(|tool| find_on_path(tool).is_none())
        .collect();
    if !missing.is_empty() {
        eprintln!("skipping: {} not on PATH", missing.join(", "));
    }
    !missing.is_empty()
}

fn config_file(socket: &Path, services: &Path) -> String {
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

fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}
