//! `wye config`: where the configuration lives and whether it is valid
//! (CFG-01).

use std::io;
use std::process::ExitCode;

use super::{Console, Context, INVALID};
use crate::cli::ConfigAction;
use crate::config_file::{self, Read};

pub fn run(
    context: &Context,
    console: &mut Console<'_>,
    action: ConfigAction,
) -> anyhow::Result<ExitCode> {
    let path = &context.paths.config;
    match action {
        ConfigAction::Path => {
            writeln!(console.out, "{}", path.display())?;
            Ok(ExitCode::SUCCESS)
        }
        ConfigAction::Check => check(console.out, path),
    }
}

/// Exit 0 when clean or absent, 1 with warnings or when unreadable, 2 when
/// the file does not parse.
fn check(out: &mut dyn io::Write, path: &std::path::Path) -> anyhow::Result<ExitCode> {
    let shown = path.display();
    Ok(match config_file::read(path) {
        Read::Missing => {
            writeln!(out, "{shown}: not found, defaults apply")?;
            ExitCode::SUCCESS
        }
        Read::Parsed(loaded) if loaded.warnings.is_empty() => {
            writeln!(out, "{shown}: OK")?;
            ExitCode::SUCCESS
        }
        Read::Parsed(loaded) => {
            for warning in &loaded.warnings {
                writeln!(out, "{shown}: {warning}")?;
            }
            ExitCode::FAILURE
        }
        Read::Invalid(error) => {
            writeln!(out, "{shown}: {error}")?;
            ExitCode::from(INVALID)
        }
        Read::Unreadable(error) => {
            writeln!(out, "{shown}: {error}")?;
            ExitCode::FAILURE
        }
    })
}
