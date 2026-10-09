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
use wye_desktop::{WYE_DESKTOP_ID, source_app};

pub use environment::Environment;
pub(crate) use environment::Snapshot;
pub(crate) use incoming::Activation;
pub(crate) use route::{PickerNeeded, Plan, plan_for, plan_start, plan_with};

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
    caller: &Caller,
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
            // The caller is the `wye open` handler (PIPE-01, DEF-08).
            handler: caller_pid(ctx, caller).await,
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
    /// `fallback`. When the chain names no app and `fallback` is unknown, the
    /// focused window stands in only if `handler`, the process that called
    /// Wye, runs in Wye's own app unit: a launcher such as KIO on Plasma 6
    /// started `wye open` as a unit of its own
    /// (`app-dev.soldunov.wye@<uuid>.service`), so its parent is
    /// `systemd --user` and says nothing about the app (DEF-08). Any other
    /// chain without an app (a timer, `systemd-run`) stays unknown.
    Pid {
        pid: u32,
        fallback: SourceApp,
        handler: Option<u32>,
    },
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
        None if probe => held_modifiers(ctx).await,
        None => Modifiers::NONE,
    }
}

/// The modifiers held now (KEY-06), for links and for the tray (TRAY-20).
pub(crate) async fn held_modifiers(ctx: &ServiceContext) -> Modifiers {
    // A call that started the service may arrive before the probes are
    // detected (`run`); give them a moment.
    ctx.probes_settled(PROBES_PATIENCE).await;
    probe_modifiers(&ctx.platform()).await
}

/// The link's source app (PIPE-01): from `hint`, reading `/proc` on a
/// blocking thread, else the focused window (13-linux-platform, step 4):
/// when the caller is the portal, which hides the app, or when the handler
/// runs in Wye's own app unit and its chain names no app (see
/// [`SourceHint::Pid`]; DEF-08). Hands `snapshot` back.
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
///
/// That is the case when the caller is the portal, and when the chain names
/// no app, the hint has no fallback, and the handler runs in Wye's own app
/// unit. A launcher that starts `wye open` in a unit of its own (KIO on
/// Plasma 6 runs it as a transient `app-dev.soldunov.wye@<uuid>.service`)
/// leaves its parent at `systemd --user`, so the walk finds nothing even
/// though the app that was clicked still has focus (PIPE-01, DEF-08). A chain
/// that names no app for another reason (a timer, `systemd-run`, a caller
/// that has exited) stays unknown: focus would credit an unrelated app.
fn from_hint(
    proc_root: &std::path::Path,
    hint: SourceHint,
    inventory: &wye_desktop::Inventory,
) -> Option<SourceApp> {
    match hint {
        SourceHint::Known(source) => Some(source),
        SourceHint::Pid {
            pid,
            fallback,
            handler,
        } => {
            let found = source::from_pid(proc_root, pid, inventory)?;
            if !found.is_unknown() {
                Some(found)
            } else if fallback.is_unknown() && in_wyes_unit(proc_root, handler) {
                None
            } else {
                Some(fallback)
            }
        }
    }
}

/// Whether process `pid` runs in Wye's own app unit.
fn in_wyes_unit(proc_root: &std::path::Path, pid: Option<u32>) -> bool {
    pid.and_then(|pid| source_app::app_unit(proc_root, pid))
        .is_some_and(|id| id.as_str() == WYE_DESKTOP_ID)
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
    caller_pid(ctx, caller).await.map_or_else(
        || SourceHint::Known(SourceApp::default()),
        |pid| SourceHint::Pid {
            pid,
            fallback: SourceApp::default(),
            // A chain that names no app stays unknown here.
            handler: None,
        },
    )
}

/// The process ID of the D-Bus peer that sent the call.
async fn caller_pid(ctx: &ServiceContext, caller: &Caller) -> Option<u32> {
    match (ctx.connection(), caller.sender.as_deref()) {
        (Some(connection), Some(sender)) => source::caller_pid(connection, sender).await,
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use wye_core::DesktopId;
    use wye_desktop::Inventory;

    use super::*;

    fn no_apps() -> Inventory {
        Inventory::from_apps(Vec::new(), Vec::new())
    }

    fn notion() -> SourceApp {
        SourceApp {
            desktop_id: DesktopId::new("notion.desktop").ok(),
            executable: None,
        }
    }

    /// A fake process under `root` with the given cgroup.
    fn process(root: &Path, pid: u32, comm: &str, parent: u32, cgroup: &str) {
        let dir = root.join(pid.to_string());
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("comm"), format!("{comm}\n")).expect("comm");
        fs::write(
            dir.join("status"),
            format!("Name:\t{comm}\nPPid:\t{parent}\n"),
        )
        .expect("status");
        fs::write(dir.join("stat"), format!("{pid} ({comm}) S {parent} 0 0\n")).expect("stat");
        fs::write(dir.join("cgroup"), format!("0::{cgroup}\n")).expect("cgroup");
        fs::write(dir.join("environ"), "").expect("environ");
    }

    const SLICE: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice";

    /// KIO on Plasma 6 starts `wye open` (pid 4000) in a transient unit of
    /// Wye's own, so its parent is `systemd --user` (pid 3006), whose chain
    /// names no app (PIPE-01, DEF-08, 13-linux-platform step 4).
    fn started_by_systemd(handler_cgroup: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        process(
            root.path(),
            3006,
            "systemd",
            1,
            "/user.slice/user@1000.service/init.scope",
        );
        process(root.path(), 4000, "wye", 3006, handler_cgroup);
        root
    }

    fn wyes_unit() -> String {
        format!("{SLICE}/app-dev.soldunov.wye@0123abcd.service")
    }

    fn pid_hint(fallback: SourceApp, handler: Option<u32>) -> SourceHint {
        SourceHint::Pid {
            pid: 3006,
            fallback,
            handler,
        }
    }

    #[test]
    fn a_handler_in_wyes_own_unit_with_no_app_in_the_chain_asks_the_focused_window() {
        let root = started_by_systemd(&wyes_unit());
        let hint = pid_hint(SourceApp::default(), Some(4000));
        assert_eq!(from_hint(root.path(), hint, &no_apps()), None);
    }

    #[test]
    fn a_handler_in_a_scope_of_wyes_own_asks_the_focused_window_too() {
        let root = started_by_systemd(&format!("{SLICE}/app-gnome-dev.soldunov.wye-777.scope"));
        let hint = pid_hint(SourceApp::default(), Some(4000));
        assert_eq!(from_hint(root.path(), hint, &no_apps()), None);
    }

    #[test]
    fn a_handler_in_a_timer_unit_stays_unknown() {
        let root = started_by_systemd(&format!("{SLICE}/backup.service"));
        let hint = pid_hint(SourceApp::default(), Some(4000));
        assert_eq!(
            from_hint(root.path(), hint, &no_apps()),
            Some(SourceApp::default())
        );
    }

    #[test]
    fn a_handler_that_is_gone_or_not_given_stays_unknown() {
        let root = started_by_systemd(&wyes_unit());
        assert_eq!(
            from_hint(
                root.path(),
                pid_hint(SourceApp::default(), None),
                &no_apps()
            ),
            Some(SourceApp::default())
        );
        assert_eq!(
            from_hint(
                root.path(),
                pid_hint(SourceApp::default(), Some(9999)),
                &no_apps()
            ),
            Some(SourceApp::default())
        );
    }

    #[test]
    fn a_known_fallback_stands_in_for_a_chain_that_names_no_app() {
        let root = started_by_systemd(&wyes_unit());
        let hint = pid_hint(notion(), Some(4000));
        assert_eq!(from_hint(root.path(), hint, &no_apps()), Some(notion()));
    }

    #[test]
    fn an_app_found_in_the_chain_wins_over_the_fallback() {
        let root = tempfile::tempdir().expect("temp dir");
        process(
            root.path(),
            100,
            "chat",
            1,
            &format!("{SLICE}/app-org.example.Chat-1.scope"),
        );
        let hint = SourceHint::Pid {
            pid: 100,
            fallback: notion(),
            handler: Some(100),
        };
        let source = from_hint(root.path(), hint, &no_apps()).expect("an app");
        assert_eq!(
            source.desktop_id,
            DesktopId::new("org.example.Chat.desktop").ok()
        );
    }

    #[test]
    fn a_known_source_is_used_as_it_is() {
        let root = started_by_systemd(&wyes_unit());
        let hint = SourceHint::Known(SourceApp::default());
        assert_eq!(
            from_hint(root.path(), hint, &no_apps()),
            Some(SourceApp::default())
        );
    }
}
