//! Routing links: `OpenLink` and `org.freedesktop.Application.Open` (IN-01,
//! IN-05, IN-07, PIPE-01, PIPE-02, PIPE-12, PIPE-15, LAUNCH-03, LAUNCH-06,
//! LAUNCH-07, PKS-05, PKS-07).
//!
//! `Open` starts source-app detection at the caller's PID; `OpenLink` takes
//! the context keys in [`wye_api::context`]. The pipeline runs on a blocking
//! thread because reading the apps (and later expansion and scripts) block.
//! A link that needs the picker goes to [`to_picker`], the one place the
//! picker broker plugs in.

mod environment;
mod incoming;
mod launch;
mod route;
mod source;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use tokio::task::JoinHandle;
use wye_api::Error;
use wye_core::{LinkRequest, Modifiers, SourceApp, Target};

pub use environment::Environment;
pub(crate) use environment::Snapshot;
pub(crate) use incoming::Activation;
pub(crate) use route::{PickerNeeded, Plan, plan, plan_for};

use self::incoming::Incoming;
use self::route::Routed;
use super::{Caller, Dict, Result};
use crate::context::ServiceContext;
use crate::platform::Platform;

/// What this topic keeps between calls.
#[derive(Debug)]
pub struct State {
    environment: RwLock<Option<Arc<Environment>>>,
    /// The link waiting for the screen to unlock (PKS-07); a newer one
    /// replaces it.
    held: Mutex<Option<Held>>,
    /// What each launch-failure notification's buttons open (LAUNCH-07).
    failed: Mutex<HashMap<u32, FailedLaunch>>,
}

/// A link held until the screen unlocks.
#[derive(Debug, Clone)]
struct Held {
    needed: PickerNeeded,
    activation: Activation,
}

/// A failed launch whose notification offers other targets.
#[derive(Debug, Clone)]
pub(crate) struct FailedLaunch {
    pub url: String,
    pub alternatives: Vec<Target>,
    pub activation: Activation,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        let environment = Environment::from_env()
            .inspect_err(|error| tracing::warn!(%error, "no session environment"))
            .ok()
            .map(Arc::new);
        Self {
            environment: RwLock::new(environment),
            held: Mutex::default(),
            failed: Mutex::default(),
        }
    }

    /// Route with `environment` from now on.
    pub(crate) fn set_environment(&self, environment: Environment) {
        *self
            .environment
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(environment));
    }

    fn environment(&self) -> Result<Arc<Environment>> {
        self.environment
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| Error::failed("the service cannot find your home directory"))
    }

    pub(crate) fn remember_failure(&self, notification: u32, failure: FailedLaunch) {
        lock(&self.failed).insert(notification, failure);
    }

    fn take_failure(&self, notification: u32) -> Option<FailedLaunch> {
        lock(&self.failed).remove(&notification)
    }

    fn hold(&self, held: Held) {
        if lock(&self.held).replace(held).is_some() {
            tracing::info!("a newer link replaces the one held for the unlock");
        }
    }

    fn release(&self) -> Option<Held> {
        lock(&self.held).take()
    }
}

/// `dev.soldunov.wye1.OpenLink`.
pub async fn open_link(
    ctx: &ServiceContext,
    _caller: &Caller,
    url: &str,
    context: &Dict,
) -> Result<()> {
    let incoming = Incoming::from_context(context)?;
    let known = incoming.source.clone().unwrap_or_default();
    let hint = match incoming.source_pid {
        // A desktop ID is as good as detection gets.
        Some(pid) if known.desktop_id.is_none() => SourceHint::Pid {
            pid,
            fallback: known,
        },
        _ => SourceHint::Known(known),
    };
    open_one(ctx, url, &incoming, hint).await
}

/// `org.freedesktop.Application.Open`: each URI enters the pipeline as a
/// handler link (IN-01). Every link is tried; the first error is returned.
pub async fn open_uris(
    ctx: &ServiceContext,
    caller: &Caller,
    uris: &[String],
    platform_data: &Dict,
) -> Result<()> {
    let incoming = Incoming::from_platform_data(platform_data);
    let hint = caller_hint(ctx, caller).await;
    let mut first_error = None;
    for uri in uris {
        if let Err(error) = open_one(ctx, uri, &incoming, hint.clone()).await {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Background work of this topic, started with the service: release held
/// links on unlock, and answer launch-failure buttons.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![
        tokio::spawn(release_on_unlock(ctx.clone())),
        tokio::spawn(answer_failure_buttons(ctx.clone())),
    ]
}

/// What is known about a link's source before the apps are read.
#[derive(Debug, Clone)]
enum SourceHint {
    /// The caller said.
    Known(SourceApp),
    /// Detect it from `pid` up (PIPE-01; step 3 uses the apps), else use
    /// `fallback`.
    Pid { pid: u32, fallback: SourceApp },
}

async fn open_one(
    ctx: &ServiceContext,
    url: &str,
    incoming: &Incoming,
    hint: SourceHint,
) -> Result<()> {
    let environment = ctx.link().environment()?;
    let (snapshot, detected) = blocking(move || {
        let snapshot = Snapshot::load(&environment);
        let detected = match hint {
            SourceHint::Known(source) => Some(source),
            SourceHint::Pid { pid, fallback } => {
                source::from_pid(&environment.proc_root, pid, &snapshot.inventory).map(|found| {
                    if found == SourceApp::default() {
                        fallback
                    } else {
                        found
                    }
                })
            }
        };
        (snapshot, detected)
    })
    .await?;
    let source = match detected {
        Some(source) => source,
        None => source::from_focus(ctx.platform().focus.as_ref()).await,
    };
    let held = match incoming.held {
        Some(held) => held,
        None => probe_modifiers(ctx.platform()).await,
    };
    let request = LinkRequest {
        source,
        held,
        screen_locked: *ctx.platform().lock.locked().borrow(),
        force: incoming.force,
        ..LinkRequest::new(url, incoming.entry)
    };
    let routed = blocking(move || route::route(&snapshot, &request)).await?;
    match routed {
        Err(rejected) => {
            launch::notify_rejected(ctx, url, &rejected).await;
            Err(Error::invalid_args(rejected.to_string()))
        }
        Ok(Routed::Launch(plan)) => launch::open(ctx, plan, &incoming.activation).await,
        Ok(Routed::Picker(needed)) => to_picker(ctx, needed, incoming.activation.clone()).await,
        Ok(Routed::Hold(needed)) => {
            tracing::info!(url, "screen locked; holding the link until it unlocks");
            ctx.link().hold(Held {
                needed,
                activation: incoming.activation.clone(),
            });
            Ok(())
        }
    }
}

/// Where a link that needs the picker goes (PIPE-13).
///
/// Seam for the picker broker: once `api::picker` can show the picker,
/// hand `needed` to it here and keep the stand-in for when the UI host
/// cannot be reached.
async fn to_picker(
    ctx: &ServiceContext,
    needed: PickerNeeded,
    activation: Activation,
) -> Result<()> {
    super::picker_fallback::open_without_picker(ctx, needed, &activation).await
}

/// Start `plan` (for topics that route links of their own).
pub(crate) async fn open_plan(
    ctx: &ServiceContext,
    plan: Plan,
    activation: &Activation,
) -> Result<()> {
    launch::open(ctx, plan, activation).await
}

/// Load a [`Snapshot`] on a blocking thread and run `work` with it.
pub(crate) async fn with_snapshot<T: Send + 'static>(
    ctx: &ServiceContext,
    work: impl FnOnce(&Snapshot) -> T + Send + 'static,
) -> Result<T> {
    let environment = ctx.link().environment()?;
    blocking(move || work(&Snapshot::load(&environment))).await
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Error::failed(format!("routing stopped: {error}")))
}

/// The modifiers held now, when the session can tell (KEY-06); none
/// otherwise.
async fn probe_modifiers(platform: &Platform) -> Modifiers {
    platform
        .modifiers
        .held()
        .await
        .map(|held| {
            let held: Vec<_> = held.into_iter().map(incoming::modifier).collect();
            Modifiers::from_slice(&held)
        })
        .unwrap_or_default()
}

/// Where to detect the source of a link `caller` handed over (IN-01).
async fn caller_hint(ctx: &ServiceContext, caller: &Caller) -> SourceHint {
    let pid = match (ctx.connection(), caller.sender.as_deref()) {
        (Some(connection), Some(sender)) => source::caller_pid(connection, sender).await,
        _ => None,
    };
    pid.map_or_else(
        || SourceHint::Known(SourceApp::default()),
        |pid| SourceHint::Pid {
            pid,
            fallback: SourceApp::default(),
        },
    )
}

/// PKS-07: show the held link's picker once the screen unlocks.
async fn release_on_unlock(ctx: ServiceContext) {
    let mut locked = ctx.platform().lock.locked();
    while locked.changed().await.is_ok() {
        if *locked.borrow_and_update() {
            continue;
        }
        if let Some(held) = ctx.link().release() {
            tracing::info!("screen unlocked; opening the held link");
            if let Err(error) = to_picker(&ctx, held.needed, held.activation).await {
                tracing::warn!(%error, "cannot open the held link");
            }
        }
    }
}

/// LAUNCH-07: open the alternative whose button was pressed.
async fn answer_failure_buttons(ctx: ServiceContext) {
    let mut presses = ctx.platform().notifier.actions();
    loop {
        let press = match presses.recv().await {
            Ok(press) => press,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                tracing::warn!(missed, "missed notification buttons");
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        };
        let Some(index) = press
            .action
            .strip_prefix(launch::OPEN_ACTION)
            .and_then(|index| index.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(failure) = ctx.link().take_failure(press.id) else {
            continue;
        };
        let Some(target) = failure.alternatives.get(index).cloned() else {
            continue;
        };
        let url = failure.url.clone();
        let opened =
            match with_snapshot(&ctx, move |snapshot| plan_for(snapshot, &target, &url)).await {
                Ok(plan) => launch::open(&ctx, plan, &failure.activation).await,
                Err(error) => Err(error),
            };
        if let Err(error) = opened {
            tracing::warn!(%error, "cannot open the alternative");
        }
    }
}
