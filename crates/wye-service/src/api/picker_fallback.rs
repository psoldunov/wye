//! The stand-in for the picker (PIPE-13) when the UI host cannot show it:
//! the link opens in the most likely browser
//! ([`wye_desktop::stand_in::choose`]), so links never go nowhere.

use wye_api::Error;
use wye_core::{Chosen, OpenOptions};

use super::Result;
use super::link::{self, Activation, PickerNeeded};
use crate::context::ServiceContext;

/// Open the link `needed` describes without a picker.
pub(crate) async fn open_without_picker(
    ctx: &ServiceContext,
    needed: PickerNeeded,
    activation: &Activation,
) -> Result<()> {
    // PIPE-14: the matched rule's script runs on the stand-in's choice too.
    let hooks = link::hooks::LinkHooks::scripts_only(ctx);
    let plan = link::with_snapshot(ctx, move |snapshot| {
        let target = wye_desktop::stand_in::choose(
            snapshot.pipeline.config(),
            &snapshot.inventory,
            snapshot.previous_default.as_ref(),
        )?;
        let chosen = Chosen {
            target,
            options: OpenOptions::default(),
        };
        Some(link::plan_with(
            snapshot,
            &needed.resolution,
            &needed.request,
            Some(chosen),
            hooks.hooks(),
        ))
    })
    .await?
    .ok_or_else(|| Error::failed("no picker could be shown and no web browser is installed"))?;
    tracing::info!(target = %plan.target, "no picker could be shown; opening in the likeliest browser");
    link::open_plan(ctx, plan, activation).await
}
