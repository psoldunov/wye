//! `wye debug …`: diagnostics for what only a real session can show.

use std::process::ExitCode;

use super::Console;
use crate::cli::DebugAction;
use crate::notice;

/// Run one diagnostic.
pub fn run(console: &mut Console<'_>, action: &DebugAction) -> ExitCode {
    match action {
        DebugAction::Probe { .. } => {
            notice::write(
                console.err,
                format_args!("wye: debug probe is not implemented yet"),
            );
            ExitCode::FAILURE
        }
    }
}
