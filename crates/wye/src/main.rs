//! `wye`: the command line and link handler that ties the routing core
//! (`wye-core`) to the Linux desktop (`wye-desktop`).
//!
//! Exit codes: 0 success, 1 runtime or launch failure, 2 a rejected link or
//! invalid input or configuration.

mod bus;
mod cli;
mod commands;
mod config_file;
mod display;
mod notice;
mod paths;
mod picker_fallback;
mod state;

use std::io;
use std::process::ExitCode;

use clap::Parser as _;

use crate::commands::Console;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let mut out = io::stdout().lock();
    // Not locked for the whole run: `wye service` logs to stderr from other
    // threads, which would wait for this lock forever.
    let mut err = io::stderr();
    let mut console = Console {
        out: &mut out,
        err: &mut err,
    };
    commands::run(cli.command, &mut console).unwrap_or_else(|error| {
        // A failure to report the failure has nowhere left to go.
        writeln!(console.err, "wye: {error:#}").ok();
        ExitCode::FAILURE
    })
}
