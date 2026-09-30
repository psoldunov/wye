//! Starting apps and moving them into their own systemd scope (LAUNCH-01 to
//! LAUNCH-06).
//!
//! [`SpawnLauncher`] starts the command through `wye_desktop`'s launch code.
//! [`SystemdScopes`] then asks the user's systemd manager to move the child
//! into a transient scope named `app-wye-<escaped desktop id>-<random>.scope`
//! (`StartTransientUnit`, as Plasma's launcher does), so the app is not a
//! part of `wye.service` and survives a restart of the service. When that
//! fails, `KillMode=process` in `wye.service` is the backstop.

use std::hash::{BuildHasher as _, RandomState};

use async_trait::async_trait;
use wye_core::DesktopId;
use wye_desktop::launch::scope::{ScopeApp, unit_name};
use zbus::zvariant::{OwnedObjectPath, Value};

use super::{LaunchCommand, Launcher, PlatformError, ScopeManager};

/// Garbage-collect the scope once its processes are gone, failed or not.
const COLLECT_MODE: &str = "inactive-or-failed";

/// Fail rather than queue when a conflicting job exists.
const JOB_MODE: &str = "fail";

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait SystemdManager {
    #[allow(
        clippy::type_complexity,
        reason = "the signature is systemd's a(sa(sv))"
    )]
    fn start_transient_unit(
        &self,
        name: &str,
        mode: &str,
        properties: &[(&str, Value<'_>)],
        aux: &[(&str, &[(&str, Value<'_>)])],
    ) -> zbus::Result<OwnedObjectPath>;
}

/// Starts processes detached from the service.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpawnLauncher;

#[async_trait]
impl Launcher for SpawnLauncher {
    async fn launch(&self, command: &LaunchCommand) -> Result<u32, PlatformError> {
        let (program, args) = command
            .argv
            .split_first()
            .ok_or_else(|| PlatformError::Failed("empty command line".to_owned()))?;
        let desktop_command = wye_desktop::LaunchCommand {
            program: program.clone(),
            args: args.to_vec(),
            remove_env: Vec::new(),
        };
        let env: Vec<(&str, Option<&str>)> = command
            .env
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_deref()))
            .collect();
        wye_desktop::launch::spawn_with_env(&desktop_command, &env)
            .map_err(|error| PlatformError::Failed(format!("cannot start {program}: {error}")))
    }
}

/// Transient scopes through the user's systemd manager.
#[derive(Debug, Clone)]
pub struct SystemdScopes {
    connection: zbus::Connection,
}

impl SystemdScopes {
    /// Scopes on the session bus `connection`, where the user manager is.
    #[must_use]
    pub fn new(connection: zbus::Connection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl ScopeManager for SystemdScopes {
    async fn adopt(&self, pid: u32, command: &LaunchCommand) -> Result<(), PlatformError> {
        let name = scope_name(command)?;
        let description = format!("{} launched by Wye", command.name);
        let manager = SystemdManagerProxy::new(&self.connection)
            .await
            .map_err(|error| failed(&name, &error))?;
        let properties = [
            ("PIDs", Value::from(vec![pid])),
            ("Description", Value::from(description.as_str())),
            ("CollectMode", Value::from(COLLECT_MODE)),
        ];
        manager
            .start_transient_unit(&name, JOB_MODE, &properties, &[])
            .await
            .map_err(|error| failed(&name, &error))?;
        Ok(())
    }
}

/// The unit name for `command`: its desktop ID, else its program.
fn scope_name(command: &LaunchCommand) -> Result<String, PlatformError> {
    let random = RandomState::new().hash_one(command.argv.as_slice());
    let desktop_id = command
        .desktop_id
        .as_deref()
        .map(DesktopId::new)
        .transpose()
        .map_err(|error| PlatformError::Failed(error.to_string()))?;
    let app = match (&desktop_id, command.argv.first()) {
        (Some(id), _) => ScopeApp::Desktop(id),
        (None, Some(program)) => ScopeApp::Program(program),
        (None, None) => return Err(PlatformError::Failed("empty command line".to_owned())),
    };
    Ok(unit_name(&app, random))
}

fn failed(name: &str, error: &zbus::Error) -> PlatformError {
    PlatformError::Failed(format!("cannot start scope {name}: {error}"))
}

/// Launching is unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLauncher;

#[async_trait]
impl Launcher for NoLauncher {
    async fn launch(&self, _command: &LaunchCommand) -> Result<u32, PlatformError> {
        Err(PlatformError::Unavailable(
            "no launcher in this session".to_owned(),
        ))
    }
}

/// Scopes are unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoScopes;

#[async_trait]
impl ScopeManager for NoScopes {
    async fn adopt(&self, _pid: u32, _command: &LaunchCommand) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable(
            "no systemd user manager".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_named_after_the_desktop_id() {
        let command = LaunchCommand {
            argv: vec!["/usr/bin/google-chrome".into()],
            desktop_id: Some("google-chrome.desktop".into()),
            ..LaunchCommand::default()
        };
        let name = scope_name(&command).expect("named");
        assert!(name.starts_with("app-wye-google\\x2dchrome-"), "{name}");
        assert_eq!(name.rsplit('.').next(), Some("scope"), "{name}");
    }

    #[test]
    fn a_custom_program_names_its_scope() {
        let command = LaunchCommand {
            argv: vec!["/opt/browser/run".into()],
            ..LaunchCommand::default()
        };
        assert!(
            scope_name(&command)
                .expect("named")
                .starts_with("app-wye-run-")
        );
    }

    #[tokio::test]
    async fn an_empty_command_is_refused() {
        let error = SpawnLauncher
            .launch(&LaunchCommand::default())
            .await
            .expect_err("refused");
        assert!(matches!(error, PlatformError::Failed(_)));
    }
}
