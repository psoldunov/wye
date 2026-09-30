//! `wye test`: the rule tester in command-line form (IN-08, DLG-TST-02).
//! Shows every step and where the link would open; nothing launches.

use std::process::ExitCode;

use wye_core::{LinkRequest, Modifiers, Pipeline, Resolution};
use wye_desktop::{Inventory, LaunchRequest, build_command};

use super::{Console, Context, INVALID};
use crate::cli::TestArgs;
use crate::{display, picker_fallback};

pub fn run(
    context: &Context,
    console: &mut Console<'_>,
    args: &TestArgs,
) -> anyhow::Result<ExitCode> {
    let pipeline = Pipeline::with_shipped_data(context.config(console.err, true));
    let inventory = context.inventory()?;
    let request = LinkRequest {
        source: args.source.clone().unwrap_or_default(),
        held: args.keys.unwrap_or(Modifiers::NONE),
        screen_locked: args.locked,
        force: args.force.force(),
        ..LinkRequest::new(args.url.as_str(), args.entry.into())
    };
    match pipeline.resolve(&request, &inventory) {
        Ok(resolution) => {
            let report = Report {
                context,
                pipeline: &pipeline,
                inventory: &inventory,
            };
            report.write(console, &resolution)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(console.err, "wye: {error}")?;
            Ok(ExitCode::from(INVALID))
        }
    }
}

/// What describing a resolution needs.
struct Report<'a> {
    context: &'a Context,
    pipeline: &'a Pipeline,
    inventory: &'a Inventory,
}

impl Report<'_> {
    fn write(&self, console: &mut Console<'_>, resolution: &Resolution) -> anyhow::Result<()> {
        writeln!(console.out, "Link: {}", resolution.original)?;
        for step in &resolution.steps {
            writeln!(console.out, "  {step}")?;
        }
        writeln!(console.out, "Result: {}", resolution.url)?;
        let target = display::target(&resolution.target, self.inventory);
        match display::options(resolution.options) {
            Some(options) => writeln!(console.out, "Opens in: {target}, {options}")?,
            None => writeln!(console.out, "Opens in: {target}")?,
        }

        let target = if picker_fallback::needed(resolution) {
            let state = self.context.state_or_default(console.err);
            let stand_in = picker_fallback::choose(self.pipeline.config(), self.inventory, &state);
            let Some(stand_in) = stand_in else {
                writeln!(
                    console.out,
                    "The picker is not available yet, and no web browser is installed."
                )?;
                return Ok(());
            };
            writeln!(
                console.out,
                "The picker is not available yet; wye open would use {}.",
                display::target(&stand_in, self.inventory)
            )?;
            stand_in
        } else {
            resolution.target.clone()
        };

        let url = self.pipeline.launch_url(&resolution.url, &target);
        let request = LaunchRequest {
            target: &target,
            url: &url,
            background: resolution.options.background,
            new_window: resolution.options.new_window,
        };
        match build_command(self.inventory, &request) {
            Ok(command) => writeln!(console.out, "Command: {}", display::command(&command))?,
            Err(error) => writeln!(console.out, "Command: none ({error})")?,
        }
        Ok(())
    }
}
