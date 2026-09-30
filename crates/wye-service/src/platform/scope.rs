//! Starting apps and moving them into their own systemd scope (LAUNCH-01 to
//! LAUNCH-06).
//!
//! Planned: spawn through `wye_desktop`, then
//! `org.freedesktop.systemd1.Manager.StartTransientUnit` on the user manager
//! with `app-wye-<escaped desktop id>-<random>.scope`. `KillMode=process` in
//! `wye.service` is the backstop. Until then nothing is started.

use async_trait::async_trait;

use super::{LaunchCommand, Launcher, PlatformError, ScopeManager};

/// Launching is unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLauncher;

#[async_trait]
impl Launcher for NoLauncher {
    async fn launch(&self, _command: &LaunchCommand) -> Result<u32, PlatformError> {
        Err(PlatformError::Unavailable(
            "launching from the service is not implemented yet".to_owned(),
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
            "systemd scopes are not implemented yet".to_owned(),
        ))
    }
}
