//! `wye-gtk`: Wye's GTK 4 / libadwaita window host for GNOME sessions.
//!
//! One resident process per session owns `dev.soldunov.wye.Gtk`, serves
//! `dev.soldunov.wye.Windows1` and `dev.soldunov.wye.PickerHost1` for the
//! service, and shows Settings, the other windows, the picker and the
//! tray-menu popup with libadwaita. It holds no routing logic: every
//! decision is the service's, reached over D-Bus. The KDE counterpart is
//! `wye-ui` (crates/wye-ui); in GNOME Shell the picker and the tray are the
//! Shell extension's (frontends/gnome-shell), and this host's picker serves
//! window managers and wlroots compositors (ADV-12).
//!
//! - [`host`]: the D-Bus interfaces and single instance (SET-04).
//! - [`route`] and [`dispatch`]: from a D-Bus call to a surface (both
//!   wye-ui's, shared from source).
//! - [`service`]: the tokio runtime and calls to the service, delivered on
//!   the GTK main context.
//! - [`app`]: the application, resources and the surface registry.
//! - [`settings`], [`about`], [`history`], [`onboarding`],
//!   [`script_editor`]: the windows.
//! - [`picker`], [`tray_menu`]: the picker and the tray-menu popup, in an
//!   [`overlay`] (a layer-shell surface where the compositor has one).
//! - [`widgets`]: the shared building blocks (BLK-01 to BLK-18).
//! - [`selftest`]: `--self-test` and `--snapshots`.
//!
//! Requirement IDs such as `SET-04` refer to the specification in
//! `docs/spec/`.

mod about;
mod app;
mod cli;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../wye-ui/src/dispatch.rs"]
mod dispatch;
mod error_text;
mod history;
mod host;
mod links;
mod onboarding;
mod overlay;
mod picker;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The routing table is the same on both hosts; its docs name Qt's side.
#[path = "../../wye-ui/src/route.rs"]
mod route;
mod rules;
mod script_editor;
mod selftest;
mod service;
mod settings;
mod surface;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[cfg(test)]
#[path = "../../wye-ui/src/test_bus.rs"]
mod test_bus;
mod tray_menu;
mod widgets;

use std::process::ExitCode;

use clap::Parser as _;
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::host::Claim;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let options = selftest::Options {
        snapshots: cli.snapshots.clone(),
        scheme: cli.scheme,
        scale: cli.scale,
    };
    let result = match (&cli.self_test, cli.self_test_child) {
        (Some(surface), _) => selftest::run(*surface, &options),
        (None, Some(surface)) => selftest::child::run(surface, options.snapshots, options.scheme),
        (None, None) => resident(&cli),
    };
    result.unwrap_or_else(|error| {
        eprintln!("wye-gtk: {error:#}");
        ExitCode::FAILURE
    })
}

/// Take the bus name and run until quit, or hand the window to the running
/// instance and exit 0 (SET-04). The D-Bus runtime lives as long as the
/// application and stops with it.
fn resident(cli: &Cli) -> anyhow::Result<ExitCode> {
    let runtime = service::new_runtime()?;
    let forward = cli.window.map(|window| (window, cli.argument.as_str()));
    let session = zbus::connection::Builder::session()?;
    let claim = runtime.block_on(host::claim(session, dispatch::global(), forward))?;
    let Claim::Owner(connection) = claim else {
        return Ok(ExitCode::SUCCESS);
    };
    let launch = app::Launch::Resident {
        window: cli.window.map(|window| (window, cli.argument.clone())),
    };
    let status = service::with_bus(runtime.handle().clone(), connection, || app::run(&launch));
    runtime.shutdown_background();
    status
}
