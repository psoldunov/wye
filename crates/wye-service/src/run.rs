//! Running one service process: connect, serve, take the name, wait.
//!
//! Single instance (DEF-04): the name is requested with `DoNotQueue`; when
//! another process owns it, [`run`] fails with [`AlreadyRunning`] and `wye
//! service` exits with [`wye_api::names::ALREADY_RUNNING_EXIT`], which
//! `wye.service` lists in `RestartPreventExitStatus=`.

use anyhow::Context as _;
use futures_lite::StreamExt as _;
use tokio::task::JoinHandle;
use wye_api::names::BUS_NAME;
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};

pub use crate::api::link::Environment;
use crate::context::ServiceContext;
use crate::platform::{Platform, session};
use crate::{api, bus, watch};

/// Another process already owns [`BUS_NAME`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("another Wye service already owns {BUS_NAME}")]
pub struct AlreadyRunning;

/// Whether `error` means another service is already running, so the caller
/// exits with [`wye_api::names::ALREADY_RUNNING_EXIT`].
#[must_use]
pub fn is_already_running(error: &anyhow::Error) -> bool {
    error.downcast_ref::<AlreadyRunning>().is_some()
}

/// How `wye service` was started.
#[derive(Debug, Clone, Default)]
pub struct ServiceOptions {
    /// Integrations to use instead of detecting the session's; tests pass
    /// fakes here.
    pub platform: Option<Platform>,
}

/// Run on the session bus until SIGINT, SIGTERM or `Quit`.
///
/// The name is claimed before the session is asked anything slow (DEF-04):
/// a link that started the service through bus activation is answered as
/// soon as the objects are served. The session probes and the shortcuts
/// portal are detected afterwards, each within a deadline
/// ([`session::probes`], [`session::shortcuts`]).
///
/// # Errors
///
/// When the session bus is unreachable, [`AlreadyRunning`] when another
/// service owns the name, or when the signal handlers cannot be installed.
pub async fn run(options: ServiceOptions) -> anyhow::Result<()> {
    let connection = zbus::Connection::session()
        .await
        .context("cannot connect to the session bus")?;
    // Asked before anything is served or started: a second service has
    // nothing useful to do.
    refuse_if_owned(&connection).await?;

    let detect = options.platform.is_none();
    let platform = options
        .platform
        .unwrap_or_else(|| session::base(&connection));
    let ctx = ServiceContext::new(platform);
    ctx.set_probes_ready(!detect);
    serve_and_claim(&connection, &ctx).await?;
    tracing::info!("serving {BUS_NAME}");
    let watchers = if detect {
        detect_then_spawn(&connection, &ctx).await
    } else {
        spawn_tasks(&ctx)
    };

    wait_for_shutdown(&ctx, &connection).await?;
    tracing::info!("shutting down");
    for watcher in watchers {
        watcher.abort();
    }
    Ok(())
}

/// Serve the objects, take the name, then start the watchers.
///
/// Split from [`run`] so tests can run the service on a private bus.
///
/// # Errors
///
/// When the objects cannot be served, or [`AlreadyRunning`].
pub async fn start(
    connection: &zbus::Connection,
    ctx: &ServiceContext,
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    serve_and_claim(connection, ctx).await?;
    Ok(spawn_tasks(ctx))
}

/// Serve the objects, then take the name.
async fn serve_and_claim(
    connection: &zbus::Connection,
    ctx: &ServiceContext,
) -> anyhow::Result<()> {
    bus::serve(connection, ctx)
        .await
        .context("cannot serve the D-Bus objects")?;
    claim_name(connection).await
}

/// Every background task, the global-shortcut listener included.
fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    spawn_session_tasks(ctx)
        .into_iter()
        .chain(api::shortcuts::spawn_tasks(ctx))
        .collect()
}

/// The background tasks that follow the session probes.
fn spawn_session_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    watch::spawn(ctx)
        .into_iter()
        .chain(api::link::spawn_tasks(ctx))
        .chain(api::tray::spawn_tasks(ctx))
        .chain(api::scripts::spawn_tasks(ctx))
        .collect()
}

/// Detect the session probes, then start the tasks that subscribe to them;
/// the shortcuts portal follows in the background, with its listener.
async fn detect_then_spawn(
    connection: &zbus::Connection,
    ctx: &ServiceContext,
) -> Vec<JoinHandle<()>> {
    let config = Environment::from_env()
        .inspect_err(
            |error| tracing::warn!(%error, "no configuration file for the preferred shortcuts"),
        )
        .ok()
        .map(|environment| environment.config);
    let reports = ctx.platform().kwin_reports.clone();
    let probes = session::probes(connection, &reports).await;
    ctx.update_platform(|platform| probes.apply(platform));
    ctx.set_probes_ready(true);
    let shortcuts = {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let shortcuts = session::shortcuts(config.as_deref()).await;
            ctx.update_platform(|platform| Platform {
                shortcuts,
                ..platform.clone()
            });
            let _listeners = AbortOnDrop(api::shortcuts::spawn_tasks(&ctx));
            std::future::pending::<()>().await;
        })
    };
    spawn_session_tasks(ctx)
        .into_iter()
        .chain(std::iter::once(shortcuts))
        .collect()
}

/// Aborts its tasks when dropped.
struct AbortOnDrop(Vec<JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

/// Route links with the files `environment` names instead of the session's
/// (tests point this at temporary directories).
pub fn use_environment(ctx: &ServiceContext, environment: Environment) {
    ctx.set_environment(environment);
}

/// Ask for [`BUS_NAME`]; refuse to run beside another service.
///
/// # Errors
///
/// [`AlreadyRunning`] when the name is taken, or the bus's own error.
pub async fn claim_name(connection: &zbus::Connection) -> anyhow::Result<()> {
    let reply = connection
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .await;
    match reply {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(()),
        Ok(RequestNameReply::InQueue | RequestNameReply::Exists) | Err(zbus::Error::NameTaken) => {
            Err(AlreadyRunning.into())
        }
        Err(error) => Err(anyhow::Error::new(error).context("cannot request the bus name")),
    }
}

async fn refuse_if_owned(connection: &zbus::Connection) -> anyhow::Result<()> {
    let bus = DBusProxy::new(connection)
        .await
        .context("cannot talk to the bus daemon")?;
    let name = BUS_NAME.try_into().context("the bus name is invalid")?;
    match bus.name_has_owner(name).await {
        Ok(true) => Err(AlreadyRunning.into()),
        // An unanswerable bus is the connection's problem, not a second
        // service's; requesting the name will tell.
        Ok(false) | Err(_) => Ok(()),
    }
}

/// Wait for SIGINT, SIGTERM, [`ServiceContext::request_shutdown`], or the
/// loss of the bus name or the bus itself: a service nobody can reach has
/// nothing left to do, and the next link starts a new one.
async fn wait_for_shutdown(
    ctx: &ServiceContext,
    connection: &zbus::Connection,
) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut term = signal(SignalKind::terminate()).context("cannot listen for SIGTERM")?;
    let mut int = signal(SignalKind::interrupt()).context("cannot listen for SIGINT")?;
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
        () = ctx.until_shutdown() => tracing::info!("quit requested"),
        () = until_unreachable(connection) => tracing::info!("lost the bus or the bus name"),
    }
    Ok(())
}

/// Resolves when [`BUS_NAME`] is lost or the connection closes.
async fn until_unreachable(connection: &zbus::Connection) {
    let lost = async {
        let bus = DBusProxy::new(connection).await?;
        let mut lost = bus.receive_name_lost().await?;
        while let Some(signal) = lost.next().await {
            if signal.args().is_ok_and(|args| args.name() == BUS_NAME) {
                break;
            }
        }
        Ok::<_, zbus::Error>(())
    };
    if let Err(error) = lost.await {
        tracing::warn!(%error, "cannot watch the bus name");
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_already_running_error_survives_the_anyhow_wrapper() {
        let error: anyhow::Error = AlreadyRunning.into();
        assert!(is_already_running(&error));
        assert!(is_already_running(&error.context("starting the service")));
        assert!(!is_already_running(&anyhow::anyhow!("no session bus")));
    }
}
