//! A private session bus for tests: `dbus-daemon` with a generated config
//! that has no `servicedir`, so nothing on it can be activated, killed when
//! dropped. The same fixture as `crates/wye-service/tests/support`.

use std::fs;
use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use tempfile::TempDir;

const DAEMON: &str = "dbus-daemon";

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

    /// The bus's address.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        // Already gone is fine; a leaked daemon is what this prevents.
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

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
