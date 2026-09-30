//! `wye-ui --self-test [SURFACE]`: load every surface (or one) offscreen,
//! feed it its fixtures, and fail on any QML warning or error.
//!
//! Each surface runs in a child process (`wye-ui --self-test-child NAME`)
//! with `QT_QPA_PLATFORM=offscreen`, `QT_FORCE_STDERR_LOGGING=1` and a
//! message pattern this module parses ([`log`]). A surface passes when the
//! child exits 0, printed the pass line, and logged no warning outside the
//! allow-list in [`log`]. The child never touches the session bus.

pub mod fixtures;
pub mod log;

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context as _;

use crate::surface::Surface;
use log::Log;

/// The hidden flag that runs one surface in this process.
pub const CHILD_FLAG: &str = "--self-test-child";

/// Longest a surface may take to load and handle its fixtures.
const TIMEOUT: Duration = Duration::from_secs(60);

/// How often the parent checks whether the child exited.
const POLL: Duration = Duration::from_millis(50);

/// Run the self-test for `surface` (a name, or `all`) and report on stdout.
///
/// # Errors
///
/// When `surface` names no surface or a child cannot be started.
pub fn run(surface: &str) -> anyhow::Result<ExitCode> {
    let surfaces = select(surface)?;
    let exe = std::env::current_exe().context("cannot find this executable")?;
    let mut out = std::io::stdout().lock();
    let mut failed = 0_usize;
    for surface in surfaces {
        let outcome = run_child(&exe, surface)?;
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
    /// Exit code; `None` when killed after [`TIMEOUT`] or by a signal.
    code: Option<i32>,
    log: Log,
}

impl Outcome {
    fn is_pass(&self) -> bool {
        self.code == Some(0) && self.log.passed() && self.log.problems().is_empty()
    }

    fn summary(&self) -> String {
        let problems = self.log.problems().len();
        match self.code {
            None => format!("did not finish within {} s", TIMEOUT.as_secs()),
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

fn run_child(exe: &Path, surface: Surface) -> anyhow::Result<Outcome> {
    let mut child = Command::new(exe)
        .args([CHILD_FLAG, surface.name()])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_MESSAGE_PATTERN", log::MESSAGE_PATTERN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start {}", exe.display()))?;
    let mut stderr = child.stderr.take().context("stderr is piped")?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).map(|_| text)
    });

    let deadline = Instant::now() + TIMEOUT;
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
        let passed = Outcome {
            code: Some(0),
            log: Log::parse("wye-log|info|js|wye-ui self-test passed: about\n"),
        };
        assert!(passed.is_pass());
        let silent = Outcome {
            code: Some(0),
            log: Log::default(),
        };
        assert!(!silent.is_pass());
        assert_eq!(silent.summary(), "exited without loading the surface");
    }
}
