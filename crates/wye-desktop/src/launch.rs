//! Launching targets (LAUNCH-01, LAUNCH-03 to LAUNCH-05, LAUNCH-08).
//!
//! Wye's desktop entry keeps `StartupNotify=true`: that is how launchers
//! hand Wye the activation token it passes on (LAUNCH-03). The cost is that
//! when Wye starts nothing that takes the token (a background launch,
//! LAUNCH-04, or a rejected link), the launcher's startup sequence is left
//! to time out, which on X11 shows a busy cursor for a few seconds.

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use wye_core::{CustomApp, DesktopId, Target};

use crate::discovery::{InstalledApp, Inventory, PrivateMode};
use crate::exec::{ExecContext, ExecError, ExecTemplate};
use crate::family::BrowserFamily;
use crate::loop_guard;

/// Wye's own desktop ID. Launching it would loop (DEF-06).
pub const WYE_DESKTOP_ID: &str = "dev.soldunov.wye.desktop";

/// Environment variables that hand focus to the launched app (LAUNCH-03).
/// Inherited by default; removed for background launches (LAUNCH-04).
pub const ACTIVATION_ENV: [&str; 2] = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"];

/// What to open and how.
#[derive(Debug, Clone, Copy)]
pub struct LaunchRequest<'a> {
    pub target: &'a Target,
    /// The link; empty starts the target without one (TRAY-20).
    pub url: &'a str,
    /// Open without taking focus (RUL-22, LAUNCH-04). The activation
    /// variables are removed, so the launcher's startup sequence for Wye
    /// times out instead of completing (see the module docs).
    pub background: bool,
    /// Force a new window (RUL-23, LAUNCH-05).
    pub new_window: bool,
}

/// A fully expanded command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: String,
    pub args: Vec<String>,
    /// Variables to remove from the inherited environment.
    pub remove_env: Vec<&'static str>,
}

/// Why a target cannot be launched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("{0} is not a concrete app; resolve it before launching")]
    NotConcrete(Target),
    #[error("refusing to open a link in Wye itself")]
    SelfLaunch,
    /// The command runs Wye or a generic opener such as `xdg-open`, which
    /// would send the link straight back to Wye (DEF-06).
    #[error("refusing to run {0}: it would send the link back to Wye")]
    LoopsBack(String),
    #[error("{0} is not installed")]
    NotInstalled(DesktopId),
    #[error("{0} runs in a terminal, which Wye does not launch")]
    Terminal(DesktopId),
    #[error("{0} has no Exec line")]
    NoExec(DesktopId),
    #[error("{id} has an invalid Exec line: {source}")]
    InvalidExec {
        id: DesktopId,
        #[source]
        source: ExecError,
    },
    #[error("Wye does not know how to open a private window in {0}")]
    NoPrivateMode(DesktopId),
    #[error("{app} has no profile {profile:?}")]
    UnknownProfile { app: DesktopId, profile: String },
    #[error("Wye does not know how to open profiles in {0}")]
    NoProfileSupport(DesktopId),
}

/// Builds the command that opens `request.url` in `request.target`.
///
/// # Errors
///
/// Returns [`LaunchError`] for Picker and Default, Wye itself, commands
/// that run Wye or a generic opener (DEF-06), apps that are missing, run in
/// a terminal or lack an `Exec` line, and private or profile targets the app
/// does not support.
pub fn build_command(
    inventory: &Inventory,
    request: &LaunchRequest<'_>,
) -> Result<LaunchCommand, LaunchError> {
    let args = match request.target {
        Target::Picker | Target::Default => {
            return Err(LaunchError::NotConcrete(request.target.clone()));
        }
        Target::Custom(CustomApp::Executable(program)) => std::iter::once(program.clone())
            .chain(Some(request.url.to_owned()).filter(|url| !url.is_empty()))
            .collect(),
        Target::App(id) | Target::Custom(CustomApp::Desktop(id)) => {
            let app = launchable(inventory, id)?;
            let flags = new_window_flag(app, request.new_window);
            expand(app, entry_exec(app)?, request.url, &flags)?
        }
        Target::Private(id) => private_args(launchable(inventory, id)?, request.url)?,
        Target::Profile { app, id } => {
            let app = launchable(inventory, app)?;
            let mut flags = profile_flags(app, id)?;
            flags.extend(new_window_flag(app, request.new_window));
            expand(app, entry_exec(app)?, request.url, &flags)?
        }
    };
    if loop_guard::runs_opener(&args)
        || loop_guard::runs_current_exe(&args, inventory.search_path())
    {
        return Err(LaunchError::LoopsBack(
            args.first().cloned().unwrap_or_default(),
        ));
    }
    let mut args = args.into_iter();
    let program = args.next().unwrap_or_default();
    Ok(LaunchCommand {
        program,
        args: args.collect(),
        remove_env: if request.background {
            ACTIVATION_ENV.to_vec()
        } else {
            Vec::new()
        },
    })
}

/// Starts the command detached from Wye: its own process group, no stdio,
/// and no waiting. A background thread reaps the child so it does not
/// linger as a zombie, and logs a failed exit. When Wye runs from its
/// `AppImage`, the child gets the session's environment, not the runtime's
/// (LAUNCH-08).
///
/// # Errors
///
/// Returns the error from starting the process.
pub fn spawn(command: &LaunchCommand) -> std::io::Result<()> {
    spawn_with_env(command, &[]).map(drop)
}

/// [`spawn`], with environment changes applied after
/// [`LaunchCommand::remove_env`]: `Some` sets a variable, `None` removes it.
/// Returns the child's process ID, for moving it into a systemd scope
/// (LAUNCH-06). The session service passes the activation token it received
/// with the link this way (LAUNCH-03).
///
/// When Wye runs from its `AppImage`, the child first loses the runtime's
/// environment (LAUNCH-08); `remove_env` and `env` apply on top of that, so
/// the activation token still wins.
///
/// # Errors
///
/// Returns the error from starting the process.
pub fn spawn_with_env(
    command: &LaunchCommand,
    env: &[(&str, Option<&str>)],
) -> std::io::Result<u32> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    for (name, value) in appimage::current_changes() {
        match value {
            Some(value) => process.env(name, value),
            None => process.env_remove(name),
        };
    }
    for name in &command.remove_env {
        process.env_remove(name);
    }
    for (name, value) in env {
        match value {
            Some(value) => process.env(name, value),
            None => process.env_remove(name),
        };
    }
    let child = process.spawn()?;
    let pid = child.id();
    let started = Instant::now();
    let program = command.program.clone();
    std::thread::spawn(move || reap(child, &program, started));
    Ok(pid)
}

/// Waits for `child`, which releases its process-table entry, and logs a
/// failed exit. The status matters because stdout and stderr are null: a
/// launch that dies at once (an `Exec` that cannot work in this
/// environment) is otherwise invisible.
fn reap(mut child: Child, program: &str, started: Instant) {
    let pid = child.id();
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(
            program,
            pid,
            %status,
            after = ?started.elapsed(),
            "launched app exited with a failure"
        ),
        Err(error) => tracing::warn!(
            program,
            pid,
            %error,
            "cannot wait for the launched app"
        ),
    }
}

fn launchable<'a>(
    inventory: &'a Inventory,
    id: &DesktopId,
) -> Result<&'a InstalledApp, LaunchError> {
    if id.as_str() == WYE_DESKTOP_ID {
        return Err(LaunchError::SelfLaunch);
    }
    let app = inventory
        .get(id)
        .ok_or_else(|| LaunchError::NotInstalled(id.clone()))?;
    if app.entry.terminal {
        return Err(LaunchError::Terminal(id.clone()));
    }
    Ok(app)
}

fn entry_exec(app: &InstalledApp) -> Result<&str, LaunchError> {
    app.entry
        .exec
        .as_deref()
        .ok_or_else(|| LaunchError::NoExec(app.id().clone()))
}

fn expand(
    app: &InstalledApp,
    exec: &str,
    url: &str,
    flags: &[String],
) -> Result<Vec<String>, LaunchError> {
    let template = ExecTemplate::parse(exec).map_err(|source| LaunchError::InvalidExec {
        id: app.id().clone(),
        source,
    })?;
    Ok(template.expand(&ExecContext::from_entry(&app.entry), url, flags))
}

/// LAUNCH-05: only Chromium- and Firefox-family browsers know `--new-window`.
fn new_window_flag(app: &InstalledApp, requested: bool) -> Vec<String> {
    app.family
        .new_window_flag()
        .filter(|_| requested)
        .map(str::to_owned)
        .into_iter()
        .collect()
}

/// DISC-05: the private action's `Exec`, or the family flag before the URL.
/// A private window is already a new window, so `new_window` is not added.
fn private_args(app: &InstalledApp, url: &str) -> Result<Vec<String>, LaunchError> {
    let action_exec = match &app.private {
        Some(PrivateMode::Action(action)) => app
            .entry
            .action(action)
            .and_then(|action| action.exec.as_deref()),
        _ => None,
    };
    if let Some(exec) = action_exec {
        return expand(app, exec, url, &[]);
    }
    let flag = app
        .family
        .private_flag()
        .ok_or_else(|| LaunchError::NoPrivateMode(app.id().clone()))?;
    expand(app, entry_exec(app)?, url, &[flag.to_owned()])
}

/// DISC-06/DISC-07: `--profile-directory=<dir>` for Chromium,
/// `--profile <absolute path>` for Firefox.
fn profile_flags(app: &InstalledApp, id: &str) -> Result<Vec<String>, LaunchError> {
    let unknown = || LaunchError::UnknownProfile {
        app: app.id().clone(),
        profile: id.to_owned(),
    };
    match app.family {
        BrowserFamily::Chromium => {
            app.profile(id).ok_or_else(unknown)?;
            Ok(vec![format!("--profile-directory={id}")])
        }
        BrowserFamily::Firefox => {
            let profile = app.profile(id).ok_or_else(unknown)?;
            Ok(vec![
                "--profile".to_owned(),
                profile.path.to_string_lossy().into_owned(),
            ])
        }
        BrowserFamily::Other => Err(LaunchError::NoProfileSupport(app.id().clone())),
    }
}

mod appimage;
pub mod scope;

#[cfg(test)]
mod tests;
