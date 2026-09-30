//! `wye clipboard [--alternative]` (IN-02 to IN-04, IN-07): open the URL on
//! the clipboard through the service. Bind it to a shortcut where global
//! shortcuts are not available (KEY-41).

use std::process::ExitCode;

use super::Console;
use super::window::report;
use crate::bus::Client;

/// Ask the service to open the clipboard's URL.
pub fn run(console: &mut Console<'_>, alternative: bool) -> ExitCode {
    report(
        console,
        Client::connect().and_then(|client| {
            client.call(|wye| async move { wye.open_clipboard(alternative).await })
        }),
    )
}
