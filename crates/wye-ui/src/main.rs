//! `wye-ui`: Wye's Qt/Kirigami UI host (docs design B).
//!
//! One resident process per session owns `dev.soldunov.wye.Ui`, serves
//! `PickerHost1` and `Windows1` for the service, and shows the picker,
//! Settings and the other windows in QML. It holds no routing logic: every
//! decision is the service's, reached over D-Bus.
//!
//! - [`host`]: the D-Bus interfaces and single instance (SET-04).
//! - [`route`] and [`dispatch`]: from a D-Bus call to a surface's QML.
//! - [`service`]: the tokio runtime and calls to the service.
//! - [`surface`]: the surfaces and their QML root files.
//! - [`picker`]: the picker's input handling and view, without Qt.
//! - [`settings`]: the Settings window's patches, menus and sheets, without Qt.
//! - [`bridge`]: the cxx-qt `QObject`s, one file per surface, and the C++ shim.
//! - [`qt_app`]: the application object, QML engine and event loop.
//! - [`selftest`]: `--self-test`, offscreen loading of every surface.
//!
//! Requirement IDs such as `SET-04` refer to the specification in
//! `docs/spec/`.

mod about;
mod bridge;
mod cli;
mod dispatch;
mod error_text;
mod history;
mod host;
mod onboarding;
mod picker;
mod qt_app;
mod route;
mod rules;
mod script_editor;
mod selftest;
mod service;
mod settings;
mod surface;
#[cfg(test)]
mod test_bus;
mod tray_menu;

use std::process::ExitCode;

use clap::Parser as _;
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;
use crate::host::Claim;
use crate::qt_app::Launch;
use crate::route::UiCommand;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let result = match (&cli.self_test, cli.self_test_child) {
        (Some(surface), _) => selftest::run(surface, cli.snapshots.as_deref()),
        (None, Some(surface)) => qt_app::run(&Launch::SelfTest {
            surface,
            snapshots: cli.snapshots.clone(),
        }),
        (None, None) => resident(&cli),
    };
    result.unwrap_or_else(|error| {
        eprintln!("wye-ui: {error:#}");
        ExitCode::FAILURE
    })
}

/// Take the bus name and run until quit, or hand the window to the running
/// instance and exit 0 (SET-04).
fn resident(cli: &Cli) -> anyhow::Result<ExitCode> {
    let dispatcher = dispatch::global();
    let forward = cli.window.map(|window| (window, cli.argument.as_str()));
    let claim = service::runtime()?.block_on(host::claim(
        zbus::connection::Builder::session()?,
        dispatcher,
        forward,
    ))?;
    let connection = match claim {
        Claim::Owner(connection) => connection,
        Claim::Forwarded => return Ok(ExitCode::SUCCESS),
    };
    service::set_connection(connection);
    if let Some(window) = cli.window {
        let command = UiCommand::ShowWindow {
            window,
            argument: cli.argument.clone(),
        };
        dispatcher.send(command.into_delivery())?;
    }
    qt_app::run(&Launch::Resident)
}
