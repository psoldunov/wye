//! A throwaway desktop for driving the real `wye` binary: temporary XDG
//! directories, fake browsers that log the links they receive, and Wye's
//! own desktop entry on demand.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

pub const ONE: &str = "fake-one.desktop";
pub const TWO: &str = "fake-two.desktop";
pub const WYE: &str = "dev.soldunov.wye.desktop";

pub struct Desktop {
    dir: TempDir,
}

impl Desktop {
    /// An empty desktop with two fake browsers, "Fake One" and "Fake Two".
    pub fn new() -> Self {
        let desktop = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        for sub in [
            "home",
            "config",
            "data/applications",
            "sysdata",
            "sysconfig",
            "state",
            "bin",
        ] {
            fs::create_dir_all(desktop.path(sub)).unwrap();
        }
        desktop.add_browser(ONE, "Fake One");
        desktop.add_browser(TWO, "Fake Two");
        desktop
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    /// A browser whose executable appends each argument to `<stem>.log`.
    pub fn add_browser(&self, id: &str, name: &str) {
        let stem = id.trim_end_matches(".desktop");
        let program = self.path("bin").join(stem);
        let log = self.log_path(id);
        fs::write(
            &program,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n", log.display()),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        self.write(
            &format!("data/applications/{id}"),
            &format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec={} %u\n\
                 MimeType=x-scheme-handler/http;x-scheme-handler/https;\n",
                program.display()
            ),
        );
    }

    /// Installs Wye's own desktop entry, as a package would.
    pub fn install_wye(&self) {
        let shipped = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/applications/dev.soldunov.wye.desktop"
        );
        fs::copy(shipped, self.path(&format!("data/applications/{WYE}"))).unwrap();
    }

    pub fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap_or_default()
    }

    pub fn config(&self, text: &str) {
        self.write("config/wye/config.toml", text);
    }

    pub fn config_path(&self) -> PathBuf {
        self.path("config/wye/config.toml")
    }

    fn log_path(&self, id: &str) -> PathBuf {
        self.path(&format!("{}.log", id.trim_end_matches(".desktop")))
    }

    /// Waits up to five seconds for the browser to log a link; launches
    /// are detached, so the log appears after `wye` exits.
    pub fn wait_for_log(&self, id: &str) -> String {
        let path = self.log_path(id);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let text = fs::read_to_string(&path).unwrap_or_default();
            if text.ends_with('\n') {
                return text;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("{id} never received a link");
    }

    /// True when the browser was never started, after a short grace
    /// period for a detached launch.
    pub fn never_launched(&self, id: &str) -> bool {
        std::thread::sleep(Duration::from_millis(300));
        !self.log_path(id).exists()
    }

    /// Runs `wye` in this desktop, with nothing from the real environment.
    pub fn wye(&self, args: &[&str]) -> Run {
        Run::from(self.command(args).output().unwrap())
    }

    /// Runs `wye` with stderr on `/dev/full`, where every write fails, as
    /// for an app that launches Wye with a broken stderr. Returns the exit
    /// code and stdout; stderr is lost.
    pub fn wye_with_broken_stderr(&self, args: &[&str]) -> Run {
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = self
            .command(args)
            .stderr(Stdio::from(full))
            .output()
            .unwrap();
        Run::from(output)
    }

    /// Runs `wye` with its session bus at `address`.
    pub fn wye_on_bus(&self, args: &[&str], address: &str) -> Run {
        let mut command = self.command(args);
        command.env("DBUS_SESSION_BUS_ADDRESS", address);
        Run::from(command.output().unwrap())
    }

    /// The environment `wye` runs in: this desktop's directories, the fake
    /// browsers first on `PATH`, and no reachable session or system bus.
    pub fn env(&self) -> Vec<(&'static str, OsString)> {
        let path = format!(
            "{}:{}",
            self.path("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        vec![
            ("PATH", path.into()),
            ("HOME", self.path("home").into()),
            ("XDG_CONFIG_HOME", self.path("config").into()),
            ("XDG_CONFIG_DIRS", self.path("sysconfig").into()),
            ("XDG_DATA_HOME", self.path("data").into()),
            ("XDG_DATA_DIRS", self.path("sysdata").into()),
            ("XDG_STATE_HOME", self.path("state").into()),
            ("XDG_CURRENT_DESKTOP", "Test".into()),
            (
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", self.path("no-bus").display()).into(),
            ),
            (
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}", self.path("no-system-bus").display()).into(),
            ),
        ]
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_wye"));
        command.args(args).env_clear().envs(self.env());
        command
    }
}

/// What a `wye` run printed and how it exited.
#[derive(Debug)]
pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl From<Output> for Run {
    fn from(output: Output) -> Self {
        Self {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl Run {
    /// Asserts the exit code, showing the output when it differs.
    pub fn expect_code(self, code: i32) -> Self {
        assert_eq!(self.code, Some(code), "{self:#?}");
        self
    }
}

/// True when `path` is a symbolic link.
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}
