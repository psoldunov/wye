//! `TestLink`'s trace: what each pipeline step did and where the link
//! would go (IN-08, DLG-TST-02). Pure; nothing is opened.

use wye_api::trace::{LinkTrace, OpenOptions, TraceDecision, TraceStep};
use wye_core::pipeline::{Decision, Step};
use wye_core::target_menu::TargetCatalog;
use wye_core::{Hooks, LinkRequest, Target};

use crate::api::history::target_name;
use crate::api::inventory::targets::spec;
use crate::api::link::Snapshot;

/// Route `request` as far as a decision, without launching.
/// `hooks` carries the short-link resolver unless `skip-network` asked
/// for none, and the transform scripts (PIPE-05, PIPE-14).
pub(crate) fn trace(
    snapshot: &Snapshot,
    catalog: &TargetCatalog,
    request: &LinkRequest,
    hooks: Hooks<'_>,
) -> LinkTrace {
    let pipeline = &snapshot.pipeline;
    let resolution = match pipeline.resolve_with(request, &*snapshot.inventory, hooks) {
        Ok(resolution) => resolution,
        Err(rejected) => {
            return LinkTrace {
                decision: TraceDecision::Rejected,
                rejected: Some(rejected.to_string()),
                ..LinkTrace::default()
            };
        }
    };
    let rule_index = match &resolution.decision {
        Decision::Rule { index, .. } => u32::try_from(*index).ok(),
        _ => None,
    };
    if !resolution.target.is_concrete() {
        return LinkTrace {
            steps: steps(&resolution.steps),
            decision: TraceDecision::Picker,
            target: Some(spec(&Target::Picker)),
            final_url: Some(resolution.url.to_string()),
            rule_index,
            ..LinkTrace::default()
        };
    }
    let finished = pipeline.finish(&resolution, request, None, hooks);
    LinkTrace {
        steps: steps(&finished.steps),
        decision: TraceDecision::Open,
        rejected: None,
        target_name: Some(target_name(&finished.target, catalog)),
        target: Some(spec(&finished.target)),
        options: OpenOptions {
            private: matches!(finished.target, Target::Private(_)),
            background: finished.options.background,
            new_window: finished.options.new_window,
        },
        final_url: Some(pipeline.launch_url(&finished.url, &finished.target)),
        rule_index,
    }
}

fn steps(steps: &[Step]) -> Vec<TraceStep> {
    steps
        .iter()
        .map(|step| TraceStep {
            kind: kind(step).to_owned(),
            text: step.to_string(),
            url: url(step),
        })
        .collect()
}

/// The step's ID for the tester's icons.
const fn kind(step: &Step) -> &'static str {
    match step {
        Step::Unwrapped { .. } => "unwrap",
        Step::ShortLinkNotExpanded
        | Step::ShortLinkExpanded { .. }
        | Step::ShortLinkFailed { .. } => "expand",
        Step::TrackingRemoved { .. } => "clean",
        Step::HttpsForced { .. } => "https",
        Step::ScriptNotRun(_)
        | Step::Transformed { .. }
        | Step::ScriptUnchanged(_)
        | Step::ScriptFailed { .. } => "script",
        Step::AlternativeKey { .. } => "alternative-key",
        Step::RuleMatched { .. } | Step::RuleSkipped { .. } => "rule",
        Step::MappingMatched { .. }
        | Step::MappingTargetMissing { .. }
        | Step::MappingSkipped { .. } => "web-app",
        Step::Fallback { .. } => "fallback",
        Step::DefaultIsPrimary { .. } => "default",
        Step::TargetMissing { .. } => "missing",
        Step::ForcedPicker { .. } => "forced-picker",
        Step::LockedScreen { .. } | Step::HeldUntilUnlock => "locked",
        Step::PickerChoice { .. } => "picker",
    }
}

/// The link after the step, for steps that change it.
fn url(step: &Step) -> Option<String> {
    match step {
        Step::Unwrapped { url, .. }
        | Step::ShortLinkExpanded { url }
        | Step::TrackingRemoved { url, .. }
        | Step::HttpsForced { url }
        | Step::Transformed { url, .. } => Some(url.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use wye_core::pipeline::EntryPoint;
    use wye_core::{Config, Pipeline};
    use wye_desktop::Inventory;

    use super::*;

    fn snapshot(config: Config) -> Snapshot {
        Snapshot {
            pipeline: std::sync::Arc::new(Pipeline::with_shipped_data(config)),
            inventory: std::sync::Arc::new(Inventory::from_apps(Vec::new(), Vec::new())),
            previous_default: None,
        }
    }

    fn traced(url: &str) -> LinkTrace {
        let request = LinkRequest::new(url, EntryPoint::Cli);
        trace(
            &snapshot(Config::default()),
            &TargetCatalog::default(),
            &request,
            Hooks::none(),
        )
    }

    #[test]
    fn a_rejected_link_says_why() {
        let traced = traced("ftp://example.com/");
        assert_eq!(traced.decision, TraceDecision::Rejected);
        assert!(traced.rejected.is_some());
    }

    #[test]
    fn without_apps_the_picker_decides_and_cleaning_is_listed() {
        let traced = traced("https://example.com/?utm_source=x");
        assert_eq!(traced.decision, TraceDecision::Picker);
        assert_eq!(traced.final_url.as_deref(), Some("https://example.com/"));
        assert!(
            traced.steps.iter().any(|step| step.kind == "clean"),
            "{:?}",
            traced.steps
        );
    }
}
