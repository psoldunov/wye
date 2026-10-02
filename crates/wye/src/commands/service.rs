//! `wye service` (DEF-04): run the session service, or with `--activate`
//! only make sure it runs (the XDG autostart entry, GEN-01).

use std::process::ExitCode;

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;
use wye_api::names::{ALREADY_RUNNING_EXIT, BUS_NAME};
use wye_service::is_already_running;
use wye_service::run::{ServiceOptions, run as run_service};

use super::Console;
use crate::notice;

/// Environment variable with the log filter, [`DEFAULT_FILTER`] when unset.
const LOG_FILTER: &str = "WYE_LOG";

/// `info`, without zbus's "Failed to populate properties cache via
/// `GetAll`" (its only `zbus::proxy` warning). ashpd builds a property-caching proxy
/// for each portal `Request` before the portal creates the object, so every
/// portal call logged it twice; the proxies are ashpd's, so their caching
/// cannot be turned off here.
const DEFAULT_FILTER: &str = "info,zbus::proxy=error";

/// Runs until SIGINT, SIGTERM or `Quit`. Exits with
/// [`ALREADY_RUNNING_EXIT`] when another service owns the name.
///
/// # Errors
///
/// When the runtime cannot start, the session bus is unreachable, or the
/// service fails.
pub fn run(console: &mut Console<'_>, activate: bool) -> anyhow::Result<ExitCode> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")?;
    if activate {
        runtime.block_on(start_by_name())?;
        return Ok(ExitCode::SUCCESS);
    }
    init_logging();
    match runtime.block_on(run_service(ServiceOptions::default())) {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(error) if is_already_running(&error) => {
            notice::write(console.err, format_args!("wye: {error}"));
            Ok(ExitCode::from(ALREADY_RUNNING_EXIT))
        }
        Err(error) => Err(error),
    }
}

/// `StartServiceByName`: the bus starts the one service instance (through
/// systemd where `SystemdService=` is honoured) and this process exits.
async fn start_by_name() -> anyhow::Result<()> {
    let connection = zbus::Connection::session()
        .await
        .context("cannot connect to the session bus")?;
    let bus = zbus::fdo::DBusProxy::new(&connection).await?;
    let name = BUS_NAME.try_into().context("the bus name is invalid")?;
    bus.start_service_by_name(name, 0)
        .await
        .with_context(|| format!("cannot start {BUS_NAME}"))?;
    Ok(())
}

/// Logs to stderr, which journald keeps for the systemd unit.
fn init_logging() {
    let filter =
        EnvFilter::try_from_env(LOG_FILTER).unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    // A subscriber installed earlier (never, in this binary) keeps logging.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .try_init();
}
