//! The subcommands and what they share.

mod browsers;
mod config;
mod default;
mod open;
mod test;

use std::io::Write;
use std::process::ExitCode;

use anyhow::Context as _;
use wye_core::{Config, DesktopId};
use wye_desktop::{Inventory, WYE_DESKTOP_ID, XdgDirs};

use crate::cli::Command;
use crate::config_file;
use crate::notice;
use crate::paths::Paths;
use crate::state::State;

/// Exit code for a rejected link, invalid input or an invalid
/// configuration. Runtime failures use [`ExitCode::FAILURE`] (1).
pub const INVALID: u8 = 2;

/// Where the commands write: results to `out`, problems to `err`.
pub struct Console<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// Runs one subcommand.
///
/// # Errors
///
/// Returns runtime failures; `main` prints them and exits with 1.
pub fn run(command: Command, console: &mut Console<'_>) -> anyhow::Result<ExitCode> {
    let context = Context::from_env()?;
    match command {
        Command::Open(args) => open::run(&context, console, &args),
        Command::Test(args) => test::run(&context, console, &args),
        Command::Browsers => browsers::run(&context, console),
        Command::Default { action } => default::run(&context, console, action.unwrap_or_default()),
        Command::Config { action } => config::run(&context, console, action.unwrap_or_default()),
    }
}

/// The environment every command works in.
pub struct Context {
    pub xdg: XdgDirs,
    pub paths: Paths,
}

impl Context {
    fn from_env() -> anyhow::Result<Self> {
        let xdg = XdgDirs::from_env().context("cannot find your home directory")?;
        let paths = Paths::from_env(&xdg);
        Ok(Self { xdg, paths })
    }

    /// The configuration, reporting problems to `err` best effort
    /// (warnings only when `warn` is set).
    pub fn config(&self, err: &mut dyn Write, warn: bool) -> Config {
        config_file::load(&self.paths.config, err, warn)
    }

    /// The state, or the empty state when it cannot be read: routing must
    /// not fail over it. The problem is reported to `err` best effort.
    pub fn state_or_default(&self, err: &mut dyn Write) -> State {
        State::load(&self.paths.state).unwrap_or_else(|error| {
            notice::write(err, format_args!("wye: {error:#}"));
            State::default()
        })
    }

    /// Every installed app except Wye itself.
    pub fn inventory(&self) -> anyhow::Result<Inventory> {
        Ok(Inventory::scan(&self.xdg, &wye_id()?))
    }
}

/// Wye's own desktop ID (DEF-06).
pub fn wye_id() -> anyhow::Result<DesktopId> {
    DesktopId::new(WYE_DESKTOP_ID).context("Wye's desktop ID is invalid")
}
