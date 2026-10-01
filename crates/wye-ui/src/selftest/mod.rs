//! `wye-ui --self-test [SURFACE]`: load every surface (or one) offscreen,
//! feed it its fixtures, and fail on any QML warning or error.
//!
//! Each surface runs in a child process (`wye-ui --self-test-child NAME`)
//! with `QT_QPA_PLATFORM=offscreen`, `QT_FORCE_STDERR_LOGGING=1` and a
//! message pattern this module parses ([`log`]). A surface passes when the
//! child exits 0, printed the pass line, and logged no warning outside the
//! allow-list in [`log`]. The child never touches the session bus.
//!
//! With `--snapshots DIR` it instead saves a PNG of every window after each
//! case ([`snapshot`]); that run is a dev tool and ignores Qt warnings.

pub mod fixtures;
pub mod log;
pub mod snapshot;

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context as _;

use crate::surface::Surface;
use log::Log;

/// The hidden flag that runs one surface in this process.
pub const CHILD_FLAG: &str = "--self-test-child";

/// The flag that turns the self-test into a snapshot run.
pub const SNAPSHOTS_FLAG: &str = "--snapshots";

/// Longest a surface may take to load and handle its fixtures.
const TIMEOUT: Duration = Duration::from_secs(60);

/// How often the parent checks whether the child exited.
const POLL: Duration = Duration::from_millis(50);

/// Run the self-test for `surface` (a name, or `all`) and report on stdout.
/// With `snapshots`, save each case's windows there instead of checking the
/// log; the children print each file they save.
///
/// # Errors
///
/// When `surface` names no surface, `snapshots` cannot be created or a
/// child cannot be started.
pub fn run(surface: &str, snapshots: Option<&Path>) -> anyhow::Result<ExitCode> {
    let surfaces = select(surface)?;
    let exe = std::env::current_exe().context("cannot find this executable")?;
    let snapshots = snapshots.map(snapshot_dir).transpose()?;
    let mut out = std::io::stdout().lock();
    let mut failed = 0_usize;
    for surface in surfaces {
        let outcome = run_child(&exe, surface, snapshots.as_deref())?;
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

/// Create `dir` and return it absolute, so the printed paths are too.
fn snapshot_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    dir.canonicalize()
        .with_context(|| format!("cannot resolve {}", dir.display()))
}

fn select(name: &str) -> anyhow::Result<Vec<Surface>> {
    if name == "all" {
        return Ok(Surface::ALL.to_vec());
    }
    let surface = Surface::from_name(name).with_context(|| {
        let names: Vec<_> = Surface::ALL.map(Surface::name).into();
        format!(
            "unknown surface {name:?}; expected all or one of {}",
            names.join(", ")
        )
    })?;
    Ok(vec![surface])
}

/// What one child did.
struct Outcome {
    /// Exit code; `None` when killed after its timeout or by a signal.
    code: Option<i32>,
    log: Log,
    /// A snapshot run: Qt warnings do not fail it.
    snapshots: bool,
    timeout: Duration,
}

impl Outcome {
    fn is_pass(&self) -> bool {
        self.code == Some(0)
            && self.log.passed()
            && (self.snapshots || self.log.problems().is_empty())
    }

    fn summary(&self) -> String {
        let problems = self.log.problems().len();
        match self.code {
            None => format!("did not finish within {} s", self.timeout.as_secs()),
            Some(0) if !self.log.passed() => "exited without loading the surface".to_owned(),
            Some(0) => format!("{problems} unexpected warning(s)"),
            Some(code) => format!("exited with {code}, {problems} unexpected warning(s)"),
        }
    }

    fn details(&self) -> Vec<String> {
        let problems = self.log.problems().into_iter().map(|message| {
            format!(
                "{:?} [{}] {}",
                message.level, message.category, message.text
            )
        });
        let foreign = self
            .log
            .foreign
            .iter()
            .map(|line| format!("(stderr) {line}"));
        problems.chain(foreign).collect()
    }
}

fn run_child(exe: &Path, surface: Surface, snapshots: Option<&Path>) -> anyhow::Result<Outcome> {
    let mut command = Command::new(exe);
    command
        .args([CHILD_FLAG, surface.name()])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_MESSAGE_PATTERN", log::MESSAGE_PATTERN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let timeout = if let Some(dir) = snapshots {
        command
            .arg(SNAPSHOTS_FLAG)
            .arg(dir)
            .envs(snapshot::environment())
            .stdout(Stdio::inherit());
        Duration::from_secs(snapshot::TIMEOUT_SECS)
    } else {
        TIMEOUT
    };
    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start {}", exe.display()))?;
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
        snapshots: snapshots.is_some(),
        timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_selects_every_surface_and_names_select_one() {
        assert_eq!(select("all").expect("all").len(), Surface::ALL.len());
        assert_eq!(select("picker").expect("one"), vec![Surface::Picker]);
        assert!(select("nope").is_err());
    }

    #[test]
    fn a_clean_run_passes_and_a_silent_one_does_not() {
        let passed = outcome(Some(0), "wye-log|info|js|wye-ui self-test passed: about\n");
        assert!(passed.is_pass());
        let silent = outcome(Some(0), "");
        assert!(!silent.is_pass());
        assert_eq!(silent.summary(), "exited without loading the surface");
    }

    #[test]
    fn only_a_snapshot_run_ignores_warnings() {
        let text = "wye-log|warning|qml|x.qml:1: oops\n\
                    wye-log|info|js|wye-ui self-test passed: about\n";
        let checked = outcome(Some(0), text);
        assert!(!checked.is_pass());
        let snapshots = Outcome {
            snapshots: true,
            ..outcome(Some(0), text)
        };
        assert!(snapshots.is_pass());
        let killed = Outcome {
            snapshots: true,
            ..outcome(None, text)
        };
        assert!(!killed.is_pass());
    }

    fn outcome(code: Option<i32>, stderr: &str) -> Outcome {
        Outcome {
            code,
            log: Log::parse(stderr),
            snapshots: false,
            timeout: TIMEOUT,
        }
    }
}
