//! A private session bus for tests: `dbus-daemon --session --nofork
//! --print-address=1`, killed when dropped.

use std::io::{BufRead as _, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// The daemon's program name.
const DAEMON: &str = "dbus-daemon";

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
