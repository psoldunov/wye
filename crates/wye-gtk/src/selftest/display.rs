//! A private headless X server for the self-test's children: `Xvfb` on the
//! first free display number, which it reports itself (`-displayfd`), killed
//! when dropped. The children draw there with GTK's X11 backend.

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};

/// The screen the server offers: room for any window at scale 2.
const SCREEN: &str = "3840x2400x24";

/// A running Xvfb.
#[derive(Debug)]
pub struct Display {
    server: Child,
    name: String,
}

impl Display {
    /// Start Xvfb; `None` when it is not on `PATH` or does not come up.
    pub fn start() -> Option<Self> {
        let spawned = Command::new("Xvfb")
            .args([
                "-displayfd",
                "1",
                "-screen",
                "0",
                SCREEN,
                "-nolisten",
                "tcp",
                "-noreset",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut server = spawned.ok()?;
        let stdout = server.stdout.take()?;
        let mut number = String::new();
        let read = BufReader::new(stdout).read_line(&mut number);
        let number = number.trim();
        if read.is_err() || number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
            // Not ours to keep: it never reported a display.
            let _ = server.kill();
            let _ = server.wait();
            return None;
        }
        Some(Self {
            name: format!(":{number}"),
            server,
        })
    }

    /// The `DISPLAY` value.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        // Already gone is fine; a leaked server is what this prevents.
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}
