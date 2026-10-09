//! Routing one link through the pipeline (PIPE-02 to PIPE-15), without IO
//! beyond what [`Snapshot`] already read. Runs on a blocking thread.

use wye_core::history::HistoryEntry;
use wye_core::pipeline::Step;
use wye_core::{Availability as _, Chosen, Hooks, LinkRequest, Rejected, Resolution, Target};
use wye_desktop::{Inventory, LaunchError, LaunchRequest, build_command};

use super::environment::Snapshot;

/// How many other targets a failed launch offers (LAUNCH-07).
const MAX_ALTERNATIVES: usize = 3;

/// Where a link goes next.
#[derive(Debug)]
pub(crate) enum Routed {
    /// A concrete app, ready to launch.
    Launch(Plan),
    /// The picker has to ask (PIPE-13).
    Picker(PickerNeeded),
    /// The picker has to ask, but the screen is locked (PKS-07).
    Hold(PickerNeeded),
}

/// A link waiting for the picker's choice.
#[derive(Debug, Clone)]
pub(crate) struct PickerNeeded {
    pub request: LinkRequest,
    pub resolution: Resolution,
}

/// One launch, or why it cannot happen.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub target: Target,
    /// The target's display name.
    pub name: String,
    /// The link as the target receives it (LAUNCH-02).
    pub url: String,
    pub background: bool,
    pub command: Result<wye_desktop::LaunchCommand, LaunchError>,
    /// Other targets to offer when the launch fails (LAUNCH-07).
    pub alternatives: Vec<Alternative>,
    /// What history records once the launch succeeded (PIPE-16); `None`
    /// for an alternative opened after a failed launch.
    pub history: Option<HistoryEntry>,
}

/// A target offered after a failed launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alternative {
    pub target: Target,
    pub name: String,
}

/// PIPE-02 to PIPE-12, then PIPE-14 when no picker is involved, with
/// `hooks` (short links, PIPE-03). Also returns the resolution's steps, for
/// the expansion notification (DLG-EXP-04).
///
/// # Errors
///
/// [`Rejected`] for links Wye does not handle (PIPE-02).
pub(crate) fn route(
    snapshot: &Snapshot,
    request: &LinkRequest,
    hooks: Hooks<'_>,
) -> Result<(Routed, Vec<Step>), Rejected> {
    let resolution = snapshot
        .pipeline
        .resolve_with(request, &*snapshot.inventory, hooks)?;
    let steps = resolution.steps.clone();
    Ok((routed(snapshot, request, &resolution, hooks), steps))
}

/// Where a resolved link goes next.
fn routed(
    snapshot: &Snapshot,
    request: &LinkRequest,
    resolution: &Resolution,
    hooks: Hooks<'_>,
) -> Routed {
    let needed = || PickerNeeded {
        request: request.clone(),
        resolution: resolution.clone(),
    };
    if resolution.hold_until_unlock {
        return Routed::Hold(needed());
    }
    if !resolution.target.is_concrete() {
        return Routed::Picker(needed());
    }
    Routed::Launch(plan_with(snapshot, resolution, request, None, hooks))
}

/// PIPE-13 to PIPE-15: the choice (none when no picker was involved), the
/// rule's script (PIPE-14, through `hooks`), then the command line.
pub(crate) fn plan_with(
    snapshot: &Snapshot,
    resolution: &Resolution,
    request: &LinkRequest,
    chosen: Option<Chosen>,
    hooks: Hooks<'_>,
) -> Plan {
    let finished = snapshot.pipeline.finish(resolution, request, chosen, hooks);
    let history = HistoryEntry::from_link(0, request, resolution, &finished);
    let url = snapshot
        .pipeline
        .launch_url(&finished.url, &finished.target);
    let command = build_command(
        &snapshot.inventory,
        &LaunchRequest {
            target: &finished.target,
            url: &url,
            background: finished.options.background,
            new_window: finished.options.new_window,
        },
    );
    Plan {
        name: name(&finished.target, &snapshot.inventory),
        alternatives: alternatives(snapshot, &finished.target),
        background: finished.options.background,
        target: finished.target,
        url,
        command,
        history: Some(history),
    }
}

/// A plan to open `url` as it is in `target`, for an alternative offered
/// after a failed launch.
pub(crate) fn plan_for(snapshot: &Snapshot, target: &Target, url: &str) -> Plan {
    let command = build_command(
        &snapshot.inventory,
        &LaunchRequest {
            target,
            url,
            background: false,
            new_window: false,
        },
    );
    Plan {
        target: target.clone(),
        name: name(target, &snapshot.inventory),
        url: url.to_owned(),
        background: false,
        command,
        alternatives: alternatives(snapshot, target),
        history: None,
    }
}

/// A plan to start `target` without a link (TRAY-20): nothing to record in
/// history, and no other target to offer when it fails, since the user
/// asked for this one.
pub(crate) fn plan_start(snapshot: &Snapshot, target: &Target) -> Plan {
    Plan {
        alternatives: Vec::new(),
        ..plan_for(snapshot, target, "")
    }
}

/// The app's name, or the target itself.
pub(crate) fn name(target: &Target, inventory: &Inventory) -> String {
    target
        .desktop_id()
        .and_then(|id| inventory.get(id))
        .map_or_else(|| target.to_string(), |app| app.entry.name.clone())
}

/// Available targets other than `failed`: primary, alternative, the shown
/// browsers, then the browser Wye replaced.
fn alternatives(snapshot: &Snapshot, failed: &Target) -> Vec<Alternative> {
    let config = snapshot.pipeline.config();
    let previous = snapshot
        .previous_default
        .iter()
        .map(|id| Target::App(id.clone()));
    let candidates = [&config.browsers.primary, &config.browsers.alternative]
        .into_iter()
        .cloned()
        .chain(
            config
                .browsers
                .shown
                .iter()
                .map(|entry| entry.target.clone()),
        )
        .chain(previous);
    let mut seen: Vec<Target> = vec![failed.clone()];
    let mut offered = Vec::new();
    for target in candidates {
        if offered.len() == MAX_ALTERNATIVES {
            break;
        }
        if seen.contains(&target)
            || !target.is_concrete()
            || !snapshot.inventory.is_available(&target)
        {
            continue;
        }
        seen.push(target.clone());
        offered.push(Alternative {
            name: name(&target, &snapshot.inventory),
            target,
        });
    }
    offered
}
