//! `wye browsers`: the installed web browsers and their profiles (TGT-05,
//! DISC-01 to DISC-05).

use std::io;
use std::process::ExitCode;

use wye_desktop::{BrowserFamily, InstalledApp, Packaging, PrivateMode};

use super::{Console, Context};

pub fn run(context: &Context, console: &mut Console<'_>) -> anyhow::Result<ExitCode> {
    let inventory = context.inventory()?;
    let out = &mut *console.out;
    let handlers = inventory.web_handlers();
    if handlers.is_empty() {
        writeln!(out, "No web browsers found")?;
    }
    for (index, app) in handlers.into_iter().enumerate() {
        if index > 0 {
            writeln!(out)?;
        }
        describe(out, app)?;
    }
    for warning in inventory.warnings() {
        writeln!(console.err, "wye: {warning}")?;
    }
    Ok(ExitCode::SUCCESS)
}

fn describe(out: &mut dyn io::Write, app: &InstalledApp) -> io::Result<()> {
    writeln!(out, "{}", app.entry.name)?;
    writeln!(out, "  ID: {}", app.id())?;
    writeln!(out, "  Family: {}", family(app.family))?;
    writeln!(out, "  Packaging: {}", packaging(app.packaging))?;
    writeln!(out, "  Private window: {}", private(app.private.as_ref()))?;
    if app.profiles.is_empty() {
        return Ok(());
    }
    writeln!(out, "  Profiles:")?;
    for profile in &app.profiles {
        writeln!(out, "    {} ({})", profile.name, profile.id)?;
    }
    Ok(())
}

fn family(family: BrowserFamily) -> &'static str {
    match family {
        BrowserFamily::Chromium => "Chromium",
        BrowserFamily::Firefox => "Firefox",
        BrowserFamily::Other => "other",
    }
}

fn packaging(packaging: Packaging) -> &'static str {
    match packaging {
        Packaging::Native => "native",
        Packaging::Flatpak => "Flatpak",
        Packaging::Snap => "Snap",
    }
}

fn private(mode: Option<&PrivateMode>) -> String {
    match mode {
        Some(PrivateMode::Action(action)) => format!("yes (desktop action {action})"),
        Some(PrivateMode::Flag(flag)) => format!("yes ({flag})"),
        None => "no".to_owned(),
    }
}
