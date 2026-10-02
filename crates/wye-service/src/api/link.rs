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
pub(crate) mod hooks;
mod incoming;
mod launch;
mod route;
mod source;

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::task::JoinHandle;
use wye_api::Error;
use wye_core::config::HeldKeys;
use wye_core::{LinkRequest, Modifiers, SourceApp, Target};

pub use environment::Environment;
pub(crate) use environment::Snapshot;
pub(crate) use incoming::Activation;
pub(crate) use route::{PickerNeeded, Plan, plan_for, plan_with};

use self::incoming::Incoming;
use self::route::Routed;
use super::{Caller, Dict, Result};
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// How many launch-failure notifications keep their buttons working
/// (LAUNCH-07). The server does not say when one closes, so the oldest is
/// forgotten instead.
const MAX_FAILED: usize = 8;

/// How long a link that needs held keys waits for the session probes of a
/// service that is still starting.
const PROBES_PATIENCE: std::time::Duration = std::time::Duration::from_millis(300);

/// How long the link held for the unlock (PKS-07) waits for the GNOME Shell
/// picker, which the Shell switches off while the screen is locked.
const SHELL_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    /// The link waiting for the screen to unlock (PKS-07); a newer one
    /// replaces it.
    held: Mutex<Option<Held>>,
    /// What each launch-failure notification's buttons open (LAUNCH-07),
    /// oldest first, at most [`MAX_FAILED`].
    failed: Mutex<VecDeque<(u32, FailedLaunch)>>,
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
        Self::default()
    }

    pub(crate) fn remember_failure(&self, notification: u32, failure: FailedLaunch) {
        let mut failed = lock(&self.failed);
        failed.retain(|(id, _)| *id != notification);
        if failed.len() == MAX_FAILED {
            failed.pop_front();
        }
        failed.push_back((notification, failure));
    }

    fn take_failure(&self, notification: u32) -> Option<FailedLaunch> {
        let mut failed = lock(&self.failed);
        let index = failed.iter().position(|(id, _)| *id == notification)?;
        failed.remove(index).map(|(_, failure)| failure)
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
/// links on unlock, answer launch-failure buttons, and the picker broker's
/// own tasks.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    [
        tokio::spawn(release_on_unlock(ctx.clone())),
        tokio::spawn(answer_failure_buttons(ctx.clone())),
    ]
    .into_iter()
    .chain(super::picker::spawn_tasks(ctx))
    .collect()
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
    let environment = ctx.environment()?;
    let snapshot = super::config::snapshot(ctx).await?;
    // BRW-03, RUL-27, ADV-11: start the probe first, alongside source
    // detection (which may ask KWin), so a quickly released Shift is still
    // seen.
    let probe = needs_probe(snapshot.pipeline.config(), incoming);
    let (held, detection) = tokio::join!(
        held_now(ctx, incoming, probe),
        detect_source(ctx, &environment, snapshot, hint)
    );
    let (snapshot, source) = detection?;
    let request = LinkRequest {
        source,
        held,
        screen_locked: *ctx.platform().lock.locked().borrow(),
        force: incoming.force,
        ..LinkRequest::new(url, incoming.entry)
    };
    let notify_expansion = snapshot
        .pipeline
        .config()
        .advanced
        .expansion
        .notify_on_failure;
    let hooks = hooks::LinkHooks::new(ctx, snapshot.pipeline.config());
    let routed = blocking(move || route::route(&snapshot, &request, hooks.hooks())).await?;
    let routed = routed.map(|(routed, steps)| {
        // DLG-EXP-04; the link carries on either way (PIPE-03).
        let ctx = ctx.clone();
        tokio::spawn(async move {
            super::expansion::notify_failures(&ctx, notify_expansion, &steps).await;
        });
        routed
    });
    act(ctx, url, incoming, routed).await
}

/// Whether the held keys are worth probing for this link: none were given,
/// `advanced.held-keys` is `auto` (ADV-11, from the cached configuration),
/// and a binding reads them (BRW-03, RUL-27).
fn needs_probe(config: &wye_core::Config, incoming: &Incoming) -> bool {
    incoming.held.is_none()
        && config.advanced.held_keys == HeldKeys::Auto
        && crate::platform::modifiers::bindings_need_modifiers(config, incoming.entry)
}

/// The held modifiers: the caller's, else probed when `probe`, else none.
async fn held_now(ctx: &ServiceContext, incoming: &Incoming, probe: bool) -> Modifiers {
    match incoming.held {
        Some(held) => held,
        None if probe => {
            // A link that started the service may arrive before the probes
            // are detected (`run`); give them a moment.
            ctx.probes_settled(PROBES_PATIENCE).await;
            probe_modifiers(&ctx.platform()).await
        }
        None => Modifiers::NONE,
    }
}

/// The link's source app (PIPE-01): from `hint`, reading `/proc` on a
/// blocking thread, else the focused window. Hands `snapshot` back.
async fn detect_source(
    ctx: &ServiceContext,
    environment: &Environment,
    snapshot: Snapshot,
    hint: SourceHint,
) -> Result<(Snapshot, SourceApp)> {
    let proc_root = environment.proc_root.clone();
    let (snapshot, detected) = blocking(move || {
        let detected = from_hint(&proc_root, hint, &snapshot.inventory);
        (snapshot, detected)
    })
    .await?;
    let source = match detected {
        Some(source) => source,
        None => source::from_focus(ctx.platform().focus.as_ref()).await,
    };
    Ok((snapshot, source))
}

/// The source `hint` names or detects; `None` when the focused window has
/// to be asked.
fn from_hint(
    proc_root: &std::path::Path,
    hint: SourceHint,
    inventory: &wye_desktop::Inventory,
) -> Option<SourceApp> {
    match hint {
        SourceHint::Known(source) => Some(source),
        SourceHint::Pid { pid, fallback } => {
            source::from_pid(proc_root, pid, inventory).map(|found| {
                if found == SourceApp::default() {
                    fallback
                } else {
                    found
                }
            })
        }
    }
}

/// Carry out where the link was routed.
async fn act(
    ctx: &ServiceContext,
    url: &str,
    incoming: &Incoming,
    routed: std::result::Result<Routed, wye_core::Rejected>,
) -> Result<()> {
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

/// Where a link that needs the picker goes (PIPE-13): the picker broker,
/// which falls back to the stand-in when the UI host cannot be reached.
async fn to_picker(
    ctx: &ServiceContext,
    needed: PickerNeeded,
    activation: Activation,
) -> Result<()> {
    super::picker::show_link(ctx, needed, activation).await
}

/// PKS-07: keep `needed` until the screen unlocks, then show the picker
/// (for a picker that was showing when the screen locked).
pub(crate) fn hold_for_unlock(ctx: &ServiceContext, needed: PickerNeeded, activation: Activation) {
    ctx.link().hold(Held { needed, activation });
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
    let snapshot = super::config::snapshot(ctx).await?;
    blocking(move || work(&snapshot)).await
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
            // GNOME Shell turns its extensions on again after the unlock:
            // its picker gets a moment to come back (ADV-12).
            super::picker::host::wait_for_shell(&ctx, SHELL_GRACE).await;
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
