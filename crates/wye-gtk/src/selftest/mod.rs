//! `wye-gtk --self-test [SURFACE]`: show every surface (or one) with its
//! fixtures and fail on any GTK, libadwaita or `GLib` warning or critical.
//!
//! Each surface runs in a child process (`wye-gtk --self-test-child NAME`,
//! [`child`]) that never touches the session bus: fixtures stand in for the
//! service. When `Xvfb` is on `PATH` (the dev shell and the flake check
//! have it) the children draw on a private headless X server ([`display`])
//! with the cairo renderer, so nothing appears on the desktop and the result
//! does not depend on the session's GPU; otherwise they use the session's
//! display. A surface passes when the child exits 0, printed the pass line,
//! and logged no warning or critical ([`log`]).
//!
//! With `--snapshots DIR` each case's windows are also saved as PNGs
//! ([`snapshot`]); `--scheme` forces light or dark, `--scale` the scale.
//!
//! The KDE host's self-test (crates/wye-ui/src/selftest/) works the same way
//! on Qt's offscreen platform.

pub mod child;
pub mod display;
pub mod fixtures;
pub mod kit;
pub mod log;
pub mod snapshot;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context as _;

use crate::cli::Scheme;
use crate::surface::Surface;
use display::Display;
use log::Log;

/// The hidden flag that runs one surface in this process.
pub const CHILD_FLAG: &str = "--self-test-child";

/// Longest a surface may take to show its fixtures.
const TIMEOUT: Duration = Duration::from_secs(60);

/// How often the parent checks whether the child exited.
const POLL: Duration = Duration::from_millis(50);

/// What the command line asked of the self-test.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub snapshots: Option<PathBuf>,
    pub scheme: Option<Scheme>,
    pub scale: Option<u8>,
}

/// Environment every child gets: no portals, no accessibility bus, no
/// dconf, the cairo renderer unless one is named, and (see [`ConfigHome`])
/// none of the user's GTK settings or CSS. Keeps the run hermetic, the look
/// GNOME's own, and the log about Wye's code.
const CHILD_ENVIRONMENT: [(&str, &str); 5] = [
    ("GTK_A11Y", "none"),
    ("NO_AT_BRIDGE", "1"),
    ("ADW_DISABLE_PORTAL", "1"),
    ("GDK_DEBUG", "no-portals"),
    ("GSETTINGS_BACKEND", "memory"),
];

/// A fontconfig file with GNOME's fonts for the children (`flake.nix` sets
/// it in the dev shell and the `gtk-selftest` check).
const FONTCONFIG_VAR: &str = "WYE_GTK_FONTCONFIG_FILE";

/// Run the self-test for `surface`, or every surface, and report on stdout.
///
/// # Errors
///
/// When the snapshot directory cannot be created or a child cannot be
/// started.
pub fn run(surface: Option<Surface>, options: &Options) -> anyhow::Result<ExitCode> {
    let surfaces = surface.map_or_else(|| Surface::ALL.to_vec(), |surface| vec![surface]);
    let exe = std::env::current_exe().context("cannot find this executable")?;
    let snapshots = options.snapshots.as_deref().map(snapshot_dir).transpose()?;
    let display = Display::start();
    let config = ConfigHome::create()?;
    let mut out = std::io::stdout().lock();
    if display.is_none() {
        writeln!(
            out,
            "note: no Xvfb on PATH; the windows open on this session's display"
        )?;
    }
    let mut failed = 0_usize;
    for surface in surfaces {
        let mut command = child_command(
            &exe,
            surface,
            display.as_ref(),
            snapshots.as_deref(),
            options,
        );
        command.env("XDG_CONFIG_HOME", &config.0);
        let outcome = run_child(command, snapshots.is_some())?;
        if outcome.is_pass() {
            writeln!(out, "ok {}", surface.name())?;
        } else {
            failed += 1;
            writeln!(out, "FAIL {}: {}", surface.name(), outcome.summary())?;
            for line in outcome.details() {
                writeln!(out, "    {line}")?;
            }
        }
    }
    if failed == 0 {
        return Ok(ExitCode::SUCCESS);
    }
    writeln!(out, "{failed} surface(s) failed")?;
    Ok(ExitCode::FAILURE)
}

/// An empty `XDG_CONFIG_HOME` for the children, removed when dropped: GTK
/// reads the user's `gtk-4.0/settings.ini` and `gtk.css` from there, and a
/// desktop's theme import (KDE writes Breeze's) restyles libadwaita and
/// makes GTK warn about it.
struct ConfigHome(PathBuf);

impl ConfigHome {
    fn create() -> anyhow::Result<Self> {
        let dir = std::env::temp_dir().join(format!("wye-gtk-selftest-{}", std::process::id()));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for ConfigHome {
    fn drop(&mut self) {
        // An empty directory of our own; failing to remove it leaves litter only.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Create `dir` and return it absolute, so the printed paths are too.
fn snapshot_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    dir.canonicalize()
        .with_context(|| format!("cannot resolve {}", dir.display()))
}

/// What one child did.
struct Outcome {
    /// Exit code; `None` when killed after its timeout or by a signal.
    code: Option<i32>,
    log: Log,
    timeout: Duration,
}

impl Outcome {
    fn is_pass(&self) -> bool {
        self.code == Some(0) && self.log.passed() && self.log.problems().is_empty()
    }

    fn summary(&self) -> String {
        let problems = self.log.problems().len();
        match self.code {
            None => format!("did not finish within {} s", self.timeout.as_secs()),
            Some(0) if !self.log.passed() => "exited without showing every case".to_owned(),
            Some(0) => format!("{problems} unexpected warning(s)"),
            Some(code) => format!("exited with {code}, {problems} unexpected warning(s)"),
        }
    }

    /// The problems, each with the code that logged it when `GLib` says,
    /// then everything else the child printed.
    fn details(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .log
            .problems()
            .into_iter()
            .map(log::Message::report)
            .collect();
        lines.extend(
            self.log
                .foreign
                .iter()
                .map(|line| format!("(stderr) {line}")),
        );
        lines
    }
}

fn child_command(
    exe: &Path,
    surface: Surface,
    display: Option<&Display>,
    snapshots: Option<&Path>,
    options: &Options,
) -> Command {
    let mut command = Command::new(exe);
    command
        .args([CHILD_FLAG, surface.name()])
        .envs(CHILD_ENVIRONMENT)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if std::env::var_os("GSK_RENDERER").is_none() {
        command.env("GSK_RENDERER", "cairo");
    }
    // GNOME's own fonts (Adwaita Sans), when the dev shell or the flake
    // check provides a fontconfig file with them.
    if let Some(fonts) = std::env::var_os(FONTCONFIG_VAR).filter(|value| !value.is_empty()) {
        command.env("FONTCONFIG_FILE", fonts);
    }
    if let Some(display) = display {
        command
            .env("GDK_BACKEND", "x11")
            .env("DISPLAY", display.name())
            .env_remove("WAYLAND_DISPLAY");
    }
    if let Some(scheme) = options.scheme {
        command.args(["--scheme", scheme.as_str()]);
    }
    if let Some(dir) = snapshots {
        command.arg("--snapshots").arg(dir).stdout(Stdio::inherit());
        if let Some(scale) = options.scale {
            command.env("GDK_SCALE", scale.to_string());
        }
    }
    command
}

fn run_child(mut command: Command, snapshots: bool) -> anyhow::Result<Outcome> {
    let timeout = if snapshots {
        Duration::from_secs(snapshot::TIMEOUT_SECS)
    } else {
        TIMEOUT
    };
    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start {}", command.get_program().to_string_lossy()))?;
    let mut stderr = child.stderr.take().context("stderr is piped")?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).map(|_| text)
    });

    let deadline = Instant::now() + timeout;
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code();
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            break None;
        }
        thread::sleep(POLL);
    };
    let text = reader
        .join()
        .map_err(|_| anyhow::anyhow!("the stderr reader panicked"))?
        .context("cannot read the child's stderr")?;
    Ok(Outcome {
        code,
        log: Log::parse(&text),
        timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_run_passes_and_a_silent_one_does_not() {
        let passed = outcome(
            Some(0),
            &marked("message", "wye-gtk self-test passed: about"),
        );
        assert!(passed.is_pass());
        let silent = outcome(Some(0), "");
        assert!(!silent.is_pass());
        assert_eq!(silent.summary(), "exited without showing every case");
    }

    #[test]
    fn a_warning_fails_the_run() {
        let text =
            marked("warning", "oops") + &marked("message", "wye-gtk self-test passed: about");
        let checked = outcome(Some(0), &text);
        assert!(!checked.is_pass());
        assert_eq!(checked.summary(), "1 unexpected warning(s)");
        let killed = outcome(None, &text);
        assert!(!killed.is_pass());
    }

    #[test]
    fn children_run_hermetically_on_the_private_display() {
        let options = Options {
            scheme: Some(Scheme::Dark),
            ..Options::default()
        };
        let command = child_command(
            Path::new("/bin/wye-gtk"),
            Surface::About,
            None,
            None,
            &options,
        );
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, [CHILD_FLAG, "about", "--scheme", "dark"]);
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.contains(&("GTK_A11Y".as_ref(), Some("none".as_ref()))));
    }

    /// A line as the child's log writer prints it.
    fn marked(level: &str, text: &str) -> String {
        let message = serde_json::json!({"level": level, "domain": "Gtk", "text": text});
        format!("wye-log {message}\n")
    }

    fn outcome(code: Option<i32>, stderr: &str) -> Outcome {
        Outcome {
            code,
            log: Log::parse(stderr),
            timeout: TIMEOUT,
        }
    }
}
