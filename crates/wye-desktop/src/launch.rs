//! Launching targets (LAUNCH-01, LAUNCH-03 to LAUNCH-05).

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use wye_core::{CustomApp, DesktopId, Target};

use crate::discovery::{InstalledApp, Inventory, PrivateMode};
use crate::exec::{ExecContext, ExecError, ExecTemplate};
use crate::family::BrowserFamily;

/// Wye's own desktop ID. Launching it would loop (DEF-06).
pub const WYE_DESKTOP_ID: &str = "dev.soldunov.wye.desktop";

/// Environment variables that hand focus to the launched app (LAUNCH-03).
/// Inherited by default; removed for background launches (LAUNCH-04).
pub const ACTIVATION_ENV: [&str; 2] = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"];

/// What to open and how.
#[derive(Debug, Clone, Copy)]
pub struct LaunchRequest<'a> {
    pub target: &'a Target,
    pub url: &'a str,
    /// Open without taking focus (RUL-22, LAUNCH-04).
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
/// Returns [`LaunchError`] for Picker and Default, Wye itself, apps that
/// are missing, run in a terminal or lack an `Exec` line, and private or
/// profile targets the app does not support.
pub fn build_command(
    inventory: &Inventory,
    request: &LaunchRequest<'_>,
) -> Result<LaunchCommand, LaunchError> {
    let args = match request.target {
        Target::Picker | Target::Default => {
            return Err(LaunchError::NotConcrete(request.target.clone()));
        }
        Target::Custom(CustomApp::Executable(program)) => {
            vec![program.clone(), request.url.to_owned()]
        }
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
/// linger as a zombie.
///
/// # Errors
///
/// Returns the error from starting the process.
pub fn spawn(command: &LaunchCommand) -> std::io::Result<()> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    for name in &command.remove_env {
        process.env_remove(name);
    }
    let mut child = process.spawn()?;
    std::thread::spawn(move || {
        // The exit status of a launched browser is of no interest; waiting
        // only releases the process-table entry.
        let _ = child.wait();
    });
    Ok(())
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

#[cfg(test)]
mod tests;
