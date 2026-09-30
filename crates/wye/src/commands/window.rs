//! `wye settings [page]` and `wye menu`: ask the service to show a window or
//! toggle the tray-menu popup (SET-04, TRAY-08, KEY-41).

use std::process::ExitCode;

use super::Console;
use crate::bus::Client;
use crate::notice;

/// `wye settings [page]`: open Settings, on `page` when given.
pub fn settings(console: &mut Console<'_>, page: Option<&str>) -> ExitCode {
    let argument = page.unwrap_or_default().to_owned();
    report(
        console,
        Client::connect().and_then(|client| {
            client.call(|wye| async move { wye.show_window("settings", &argument).await })
        }),
    )
}

/// `wye menu`: toggle the tray-menu popup (TRAY-08; bind it to a shortcut,
/// KEY-41).
pub fn menu(console: &mut Console<'_>) -> ExitCode {
    report(
        console,
        Client::connect()
            .and_then(|client| client.call(|wye| async move { wye.toggle_menu().await })),
    )
}

/// Exit 0 on success; otherwise print why and exit 1.
pub fn report(console: &mut Console<'_>, result: Result<(), crate::bus::CallError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            notice::write(console.err, format_args!("wye: {error}"));
            ExitCode::FAILURE
        }
    }
}
