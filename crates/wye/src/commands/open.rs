//! `wye open` (IN-01, IN-07): route each link and launch it (PIPE-01 to
//! PIPE-15).

use std::path::Path;
use std::process::ExitCode;

use anyhow::Context as _;
use wye_core::{EntryPoint, LinkRequest, Pipeline, Resolution, Target};
use wye_desktop::{Inventory, LaunchRequest, build_command, source_app, spawn};

use super::{Console, Context, INVALID};
use crate::cli::OpenArgs;
use crate::{display, picker_fallback};

/// Opens every link. Rejected links and failed launches are reported
/// without stopping the others; the exit code is 2 when any link was
/// rejected, else 1 when any launch failed.
pub fn run(
    context: &Context,
    console: &mut Console<'_>,
    args: &OpenArgs,
) -> anyhow::Result<ExitCode> {
    if args.urls.is_empty() {
        // The app-menu entry runs `wye open` without a link. TODO(TRAY-05):
        // open Settings instead.
        writeln!(console.err, "wye: no link given")?;
        return Ok(ExitCode::SUCCESS);
    }
    // Links clicked in apps have no terminal to show warnings in.
    let debug = std::env::var_os("WYE_DEBUG").is_some();
    let pipeline = Pipeline::with_shipped_data(context.config(console.err, debug)?);
    let inventory = context.inventory()?;
    let source = source_app::detect(Path::new("/proc"), std::os::unix::process::parent_id());
    let opener = Opener {
        context,
        pipeline: &pipeline,
        inventory: &inventory,
    };

    let mut rejected = false;
    let mut failed = false;
    for url in &args.urls {
        // TODO(PIPE-01): held modifiers and the lock state need the session
        // service; until then no key is held and the screen is unlocked.
        let request = LinkRequest {
            source: source.clone(),
            force: args.force.force(),
            ..LinkRequest::new(url.as_str(), EntryPoint::Handler)
        };
        match pipeline.resolve(&request, &inventory) {
            Ok(resolution) => {
                if let Err(error) = opener.launch(console, &resolution) {
                    // TODO(LAUNCH-07): report launch failures as notifications.
                    writeln!(console.err, "wye: {error:#}")?;
                    failed = true;
                }
            }
            Err(error) => {
                writeln!(console.err, "wye: {error}")?;
                rejected = true;
            }
        }
    }
    Ok(if rejected {
        ExitCode::from(INVALID)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// What launching a resolved link needs.
struct Opener<'a> {
    context: &'a Context,
    pipeline: &'a Pipeline,
    inventory: &'a Inventory,
}

impl Opener<'_> {
    /// Launches a resolved link, standing in for the picker when needed.
    fn launch(&self, console: &mut Console<'_>, resolution: &Resolution) -> anyhow::Result<()> {
        let target = if picker_fallback::needed(resolution) {
            self.stand_in(console)?
        } else {
            resolution.target.clone()
        };
        let url = self.pipeline.launch_url(&resolution.url, &target);
        let command = build_command(
            self.inventory,
            &LaunchRequest {
                target: &target,
                url: &url,
                background: resolution.options.background,
                new_window: resolution.options.new_window,
            },
        )?;
        spawn(&command).with_context(|| format!("cannot start {}", command.program))
    }

    /// The interim picker stand-in, announced on stderr.
    fn stand_in(&self, console: &mut Console<'_>) -> anyhow::Result<Target> {
        let state = self.context.state_or_default(console.err)?;
        let target = picker_fallback::choose(self.pipeline.config(), self.inventory, &state)
            .context("the picker is not available yet and no web browser is installed")?;
        writeln!(
            console.err,
            "wye: the picker is not available yet; opening in {}",
            display::target_name(&target, self.inventory)
        )?;
        Ok(target)
    }
}
