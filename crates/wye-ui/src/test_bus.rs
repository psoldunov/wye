//! A private session bus for tests that need one.

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};

/// A private `dbus-daemon`, killed on drop; `None` (the test skips)
/// when the program is not on `PATH`.
pub struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    pub fn start() -> Option<Self> {
        let spawned = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn();
        let Ok(mut daemon) = spawned else {
            eprintln!("skipping: dbus-daemon is not on PATH");
            return None;
        };
        let stdout = daemon.stdout.take().expect("stdout is piped");
        let mut address = String::new();
        BufReader::new(stdout)
            .read_line(&mut address)
            .expect("dbus-daemon prints its address");
        Some(Self {
            daemon,
            address: address.trim().to_owned(),
        })
    }

    pub fn builder(&self) -> zbus::connection::Builder<'_> {
        zbus::connection::Builder::address(self.address.as_str()).expect("address parses")
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        // Already gone is fine; a leaked daemon is what this prevents.
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
