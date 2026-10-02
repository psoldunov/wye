//! Calls to the session's picker and window hosts (ADV-12). The order comes
//! from [`super::frontend`]: a host that is neither running nor installed
//! is skipped, and a picker host that fails hands the request to the next
//! one, so no link is lost. A window host that answers with an error keeps
//! it; only a host that cannot be reached hands a window over. A whole chain
//! of hosts shares one deadline, so the stand-in (PIPE-13) never waits
//! longer than a single host did.

use std::time::Duration;

use futures_lite::StreamExt as _;
use tokio::time::Instant;
use wye_api::Error;
use wye_api::actions::Window;
use wye_api::names::{GNOME_BUS_NAME, PICKER_HOST_INTERFACE};
use wye_api::proxy::{PickerHost1Proxy, Windows1Proxy};
use wye_core::config::Frontend;
use zbus::DBusError as _;
use zbus::fdo::DBusProxy;
use zbus::names::WellKnownName;

use super::frontend::{self, Host};
use crate::api::link;
use crate::context::ServiceContext;
use crate::platform::gnome_shell;

/// Longest a call to the UI may take, bus activation of a cold `wye-ui`
/// included; also the budget of a whole chain of hosts.
const UI_DEADLINE: Duration = Duration::from_secs(10);

/// Errors of a host that runs but does not serve the interface asked for,
/// such as a GTK host that only serves `Windows1`.
const NOT_SERVED: [&str; 3] = [
    "org.freedesktop.DBus.Error.UnknownInterface",
    "org.freedesktop.DBus.Error.UnknownMethod",
    "org.freedesktop.DBus.Error.UnknownObject",
];

/// Whether this is a GNOME Shell session (ADV-12), asked of the bus each
/// time ([`gnome_shell::is_gnome_session`]).
pub(super) async fn is_gnome(ctx: &ServiceContext) -> bool {
    match ctx.connection() {
        Some(connection) => gnome_shell::is_gnome_session(connection).await,
        None => false,
    }
}

/// `advanced.frontend` (ADV-12) from the cached configuration; `auto` when
/// it cannot be read, so a broken file still reaches a picker.
pub(super) async fn configured_frontend(ctx: &ServiceContext) -> Frontend {
    let frontend =
        link::with_snapshot(ctx, |snapshot| snapshot.pipeline.config().advanced.frontend).await;
    frontend.unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read the frontend setting; using auto");
        Frontend::Auto
    })
}

async fn picker_hosts(ctx: &ServiceContext) -> &'static [Host] {
    frontend::picker_hosts(configured_frontend(ctx).await, is_gnome(ctx).await)
}

/// PICK-25: whether `bus_name` leaving the bus restarts the warm-up.
pub(super) async fn rewarms(ctx: &ServiceContext, frontend: Frontend, bus_name: &str) -> bool {
    frontend::rewarms(frontend, is_gnome(ctx).await, bus_name)
}

fn connection(ctx: &ServiceContext) -> Result<&zbus::Connection, Error> {
    ctx.connection()
        .ok_or_else(|| Error::Unavailable("the service is not on the session bus".to_owned()))
}

fn bus_name(host: Host) -> Result<WellKnownName<'static>, Error> {
    WellKnownName::try_from(host.bus_name()).map_err(|error| zbus::Error::from(error).into())
}

/// A UI host that cannot be reached (not installed, not activatable, gone)
/// is `Unavailable`; what the UI itself answered stays as it is.
fn unreachable(error: Error) -> Error {
    match error {
        Error::Bus(error) => Error::Unavailable(format!("the UI host cannot be reached: {error}")),
        other => other,
    }
}

fn not_served(error: &Error) -> bool {
    match error {
        Error::Bus(zbus::Error::MethodError(name, _, _)) => NOT_SERVED.contains(&name.as_str()),
        Error::Bus(zbus::Error::FDO(error)) => NOT_SERVED.contains(&error.name().as_str()),
        _ => false,
    }
}

async fn within<T>(
    deadline: Duration,
    what: &str,
    call: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::time::timeout(deadline, call)
        .await
        .map_err(|_| Error::Unavailable(format!("the UI did not answer {what} in time")))?
        .map_err(unreachable)
}

/// What is left of a chain's [`UI_DEADLINE`].
#[derive(Debug, Clone, Copy)]
struct Budget {
    end: Instant,
}

impl Budget {
    fn start() -> Self {
        Self {
            end: Instant::now() + UI_DEADLINE,
        }
    }

    fn left(self) -> Duration {
        self.end.saturating_duration_since(Instant::now())
    }

    /// What a host with others behind it may take.
    fn share(self) -> Duration {
        share_of(self.left())
    }
}

/// Half of what is left: a host that hangs leaves the other half to the
/// hosts behind it, and the last one takes all that remains.
fn share_of(left: Duration) -> Duration {
    left / 2
}

async fn owned(connection: &zbus::Connection, host: Host) -> Result<bool, Error> {
    Ok(DBusProxy::new(connection)
        .await?
        .name_has_owner(bus_name(host)?.into())
        .await?)
}

/// Whether `host` runs now or D-Bus can start it. A host that cannot be
/// checked within `deadline` counts as absent.
async fn available(connection: &zbus::Connection, host: Host, deadline: Duration) -> bool {
    let checked = within(deadline, "checking the UI host", async {
        if owned(connection, host).await? {
            return Ok(true);
        }
        if !host.activatable() {
            return Ok(false);
        }
        Ok(DBusProxy::new(connection)
            .await?
            .list_activatable_names()
            .await?
            .iter()
            .any(|name| name.as_str() == host.bus_name()))
    })
    .await;
    checked.unwrap_or_else(|error| {
        tracing::warn!(%error, host = host.label(), "cannot check the UI host; skipping it");
        false
    })
}

/// The hosts that own their name now. A host that cannot be checked counts
/// as not running, so one failed lookup does not stop the others.
async fn running_hosts(connection: &zbus::Connection) -> Vec<Host> {
    let mut running = Vec::new();
    for host in Host::ALL {
        match within(UI_DEADLINE, "checking the UI host", owned(connection, host)).await {
            Ok(true) => running.push(host),
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, host = host.label(), "cannot check the UI host; skipping it");
            }
        }
    }
    running
}

/// When a host that was called hands the call to the next one.
#[derive(Debug, Clone, Copy)]
enum Handover {
    /// On any error: a picker must reach someone (PIPE-13).
    OnAnyError,
    /// Only when the host cannot be reached; what it answered stands.
    OnUnreachable,
}

impl Handover {
    fn applies(self, error: &Error) -> bool {
        match self {
            Self::OnAnyError => true,
            Self::OnUnreachable => matches!(error, Error::Unavailable(_)),
        }
    }
}

/// Call `hosts` in order until one serves `what`; the host that did. Hosts
/// that are neither running nor installed are skipped; the last one is
/// always called, so its error is the one reported. The chain shares one
/// [`UI_DEADLINE`]: each host before the last gets half of what is left, the
/// last all of it (with one host, the whole deadline, as before ADV-12).
async fn first_serving<T, F, Fut>(
    connection: &zbus::Connection,
    hosts: &[Host],
    what: &str,
    handover: Handover,
    call: F,
) -> Result<(T, Host), Error>
where
    F: Fn(Host) -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let Some((&last, earlier)) = hosts.split_last() else {
        return Err(Error::Unavailable(format!("no UI host for {what}")));
    };
    let budget = Budget::start();
    for &host in earlier {
        if !available(connection, host, budget.share()).await {
            if host.activatable() {
                tracing::warn!(
                    host = host.label(),
                    "{what}: the host is not installed; trying the next frontend"
                );
            }
            continue;
        }
        let label = format!("{what} ({})", host.label());
        match within(budget.share(), &label, call(host)).await {
            Ok(value) => return Ok((value, host)),
            Err(error) if handover.applies(&error) => {
                tracing::warn!(%error, host = host.label(), "{what} failed; trying the next frontend");
            }
            Err(error) => return Err(error),
        }
    }
    let label = format!("{what} ({})", last.label());
    let value = within(budget.left(), &label, call(last)).await?;
    Ok((value, last))
}

async fn picker_proxy(
    connection: &zbus::Connection,
    host: Host,
) -> zbus::Result<PickerHost1Proxy<'static>> {
    PickerHost1Proxy::builder(connection)
        .destination(host.bus_name())?
        .path(host.path())?
        .build()
        .await
}

/// `PickerHost1.ShowPicker` (PICK-01, PICK-27) on the first host that
/// shows it (ADV-12); the host that did.
pub(crate) async fn show_picker(
    ctx: &ServiceContext,
    request_id: &str,
    request: &str,
) -> Result<Host, Error> {
    let connection = connection(ctx)?;
    let hosts = picker_hosts(ctx).await;
    let ((), host) = first_serving(
        connection,
        hosts,
        "ShowPicker",
        Handover::OnAnyError,
        |host| async move {
            picker_proxy(connection, host)
                .await?
                .show_picker(request_id, request)
                .await
        },
    )
    .await?;
    Ok(host)
}

/// `ClosePicker` on `host`; a host without a picker has none to close.
async fn close_on(
    connection: &zbus::Connection,
    host: Host,
    request_id: &str,
) -> Result<(), Error> {
    match picker_proxy(connection, host)
        .await?
        .close_picker(request_id)
        .await
    {
        Err(error) if not_served(&error) => Ok(()),
        result => result,
    }
}

/// `PickerHost1.ClosePicker` on every running host: a request may have
/// reached any of them before the frontend changed or a host left. With
/// none running, the preferred host is asked, so a request it is starting
/// for does not show late.
pub(crate) async fn close_picker(ctx: &ServiceContext, request_id: &str) -> Result<(), Error> {
    let connection = connection(ctx)?;
    let running = running_hosts(connection).await;
    if running.is_empty() {
        let hosts = picker_hosts(ctx).await;
        let close = |host| close_on(connection, host, request_id);
        return first_serving(
            connection,
            hosts,
            "ClosePicker",
            Handover::OnAnyError,
            close,
        )
        .await
        .map(|((), _)| ());
    }
    close_each(connection, &running, request_id).await
}

/// `ClosePicker` on the running hosts `which` accepts, never starting one:
/// a request that a newer one, shown on another host, replaced (PICK-27).
pub(crate) async fn close_running(
    ctx: &ServiceContext,
    request_id: &str,
    which: impl Fn(Host) -> bool,
) -> Result<(), Error> {
    let connection = connection(ctx)?;
    let mut running = running_hosts(connection).await;
    running.retain(|&host| which(host));
    close_each(connection, &running, request_id).await
}

async fn close_each(
    connection: &zbus::Connection,
    hosts: &[Host],
    request_id: &str,
) -> Result<(), Error> {
    let mut closed = Ok(());
    for &host in hosts {
        let label = format!("ClosePicker ({})", host.label());
        let result = within(UI_DEADLINE, &label, close_on(connection, host, request_id)).await;
        closed = closed.and(result);
    }
    closed
}

/// `PickerHost1.ShowMenu` on the first host that shows it (ADV-12).
pub(crate) async fn show_menu(ctx: &ServiceContext, menu: &str) -> Result<(), Error> {
    let connection = connection(ctx)?;
    let hosts = picker_hosts(ctx).await;
    first_serving(
        connection,
        hosts,
        "ShowMenu",
        Handover::OnAnyError,
        |host| async move { picker_proxy(connection, host).await?.show_menu(menu).await },
    )
    .await
    .map(|((), _)| ())
}

/// `Windows1.ShowWindow` on the first window host that can be reached
/// (ADV-12). An installed host that fails reports it rather than letting
/// the other frontend hide the failure.
pub(crate) async fn show_window(
    ctx: &ServiceContext,
    window: Window,
    argument: &str,
) -> Result<(), Error> {
    let connection = connection(ctx)?;
    let hosts = frontend::window_hosts(configured_frontend(ctx).await, is_gnome(ctx).await);
    first_serving(
        connection,
        hosts,
        "ShowWindow",
        Handover::OnUnreachable,
        |host| async move {
            Windows1Proxy::builder(connection)
                .destination(host.bus_name())?
                .path(host.path())?
                .build()
                .await?
                .show_window(window.as_str(), argument)
                .await
        },
    )
    .await
    .map(|((), _)| ())
}

/// Whether a running `host` serves `PickerHost1`: the GTK host may serve
/// `Windows1` alone. A host that cannot be asked counts as serving it, so
/// no second toolkit starts on a guess.
pub(super) async fn serves_picker(ctx: &ServiceContext, host: Host) -> bool {
    let Some(connection) = ctx.connection() else {
        return true;
    };
    let introspected = within(UI_DEADLINE, "checking the picker host", async {
        Ok(zbus::fdo::IntrospectableProxy::builder(connection)
            .destination(host.bus_name())?
            .path(host.path())?
            .build()
            .await?
            .introspect()
            .await?)
    })
    .await;
    match introspected {
        Ok(xml) => xml.contains(&format!("\"{PICKER_HOST_INTERFACE}\"")),
        Err(error) => {
            tracing::warn!(%error, host = host.label(), "cannot check the picker host");
            true
        }
    }
}

/// The host to start ahead of the next picker (decision 2, ADV-12): the
/// first one that would show it, or none when that is the running GNOME
/// Shell, which needs nothing started. A running host without `PickerHost1`
/// is passed over, as [`show_picker`] passes it over, and so is `besides`
/// (a host that did not start or serves no picker).
pub(crate) async fn warm_host(
    ctx: &ServiceContext,
    frontend: Frontend,
    besides: Option<Host>,
) -> Option<Host> {
    let connection = ctx.connection()?;
    let hosts = frontend::picker_hosts(frontend, is_gnome(ctx).await);
    let (&last, earlier) = hosts.split_last()?;
    for &host in earlier {
        if besides == Some(host) || !available(connection, host, UI_DEADLINE).await {
            continue;
        }
        if !host.activatable() {
            return None;
        }
        let running = owned(connection, host).await.unwrap_or(false);
        if !running || serves_picker(ctx, host).await {
            return Some(host);
        }
        tracing::info!(
            host = host.label(),
            "the host serves no picker; keeping the next frontend ready"
        );
    }
    (last.activatable() && besides != Some(last)).then_some(last)
}

/// Start `host` ahead of the first picker (decision 2), so the first link
/// does not wait for its toolkit to load.
pub(crate) async fn activate(ctx: &ServiceContext, host: Host) -> Result<(), Error> {
    let connection = connection(ctx)?;
    within(UI_DEADLINE, "StartServiceByName", async {
        DBusProxy::new(connection)
            .await?
            .start_service_by_name(bus_name(host)?, 0)
            .await?;
        Ok(())
    })
    .await
}

/// PKS-07 on GNOME: the Shell switches its extensions off while the screen
/// is locked and on again after the unlock, so the picker of a link held
/// for the unlock waits up to `grace` for the extension's name, instead of
/// going to the next host, when the Shell heads the hosts.
pub(crate) async fn wait_for_shell(ctx: &ServiceContext, grace: Duration) {
    let Some(connection) = ctx.connection() else {
        return;
    };
    if picker_hosts(ctx).await.first() != Some(&Host::Shell) || !is_gnome(ctx).await {
        return;
    }
    let back = async {
        // Subscribed before asking, so an owner that comes in between counts.
        let mut owners = DBusProxy::new(connection)
            .await?
            .receive_name_owner_changed_with_args(&[(0, GNOME_BUS_NAME)])
            .await?;
        if owned(connection, Host::Shell).await? {
            return Ok(());
        }
        while let Some(change) = owners.next().await {
            if change.args().is_ok_and(|args| args.new_owner().is_some()) {
                return Ok(());
            }
        }
        Ok::<_, Error>(())
    };
    match tokio::time::timeout(grace, back).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!(%error, "cannot wait for the GNOME Shell picker"),
        Err(_) => tracing::info!("the GNOME Shell picker did not come back after the unlock"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_of_hanging_hosts_stays_within_one_deadline() {
        // ADV-12 with PIPE-13: Shell, GTK and Qt all hanging still reach
        // the stand-in within the deadline a single host had.
        let mut left = UI_DEADLINE;
        let mut spent = Duration::ZERO;
        for _ in 0..2 {
            let share = share_of(left);
            assert!(share > Duration::ZERO);
            spent += share;
            left -= share;
        }
        spent += left;
        assert_eq!(spent, UI_DEADLINE);
        assert_eq!(share_of(UI_DEADLINE), Duration::from_secs(5));
    }
}
