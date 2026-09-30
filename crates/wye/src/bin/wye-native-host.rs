//! `wye-native-host`: the browser extension's native-messaging host
//! (BEXT-01 to BEXT-06).
//!
//! Not implemented yet. The browser starts it and exchanges length-prefixed
//! JSON messages over stdin and stdout; until the host exists it says so on
//! stderr (which the browser logs) and exits with a failure.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Nothing useful can be done when stderr fails.
    writeln!(std::io::stderr(), "wye-native-host: not implemented yet").ok();
    ExitCode::FAILURE
}
