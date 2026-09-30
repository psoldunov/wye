//! `wye-native-host`: the browser extension's native-messaging host
//! (BEXT-04 to BEXT-06). See `crates/wye/src/native_host/`.
//!
//! The browser starts it with its own arguments and talks to it over stdin
//! and stdout; anything written to stderr ends up in the browser's log.
//! `wye-native-host --install` writes the host manifests of every detected
//! browser, `--remove` deletes them (as `wye extension install|remove` do).

#[path = "../native_host/mod.rs"]
mod native_host;

use std::ffi::OsString;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use native_host::install;
use native_host::message::Link;
use native_host::{Service, context, serve};
use wye_desktop::xdg::XdgDirs;

/// What the command line asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Talk to the browser on stdin and stdout.
    Serve,
    /// Write the manifests.
    Install,
    /// Delete the manifests.
    Remove,
}

const INSTALL: &str = "--install";
const REMOVE: &str = "--remove";

/// The mode `args` (without the program name) ask for. The browsers start
/// the host with their own arguments (Chromium: the extension's origin;
/// Firefox: the manifest path and the extension ID), so only these two
/// flags, as the first argument, mean anything else.
fn mode(args: &[OsString]) -> Mode {
    match args.first().and_then(|arg| arg.to_str()) {
        Some(INSTALL) => Mode::Install,
        Some(REMOVE) => Mode::Remove,
        _ => Mode::Serve,
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let result = match mode(&args) {
        Mode::Serve => run_host(),
        Mode::Install => with_xdg(|xdg| {
            let exe = std::env::current_exe().ok();
            install::install(&mut std::io::stdout().lock(), xdg, exe.as_deref())
        }),
        Mode::Remove => with_xdg(|xdg| install::remove(&mut std::io::stdout().lock(), xdg)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Nothing more can be done when stderr fails too.
            writeln!(std::io::stderr(), "wye-native-host: {message}").ok();
            ExitCode::FAILURE
        }
    }
}

/// Serve the browser until it closes stdin.
fn run_host() -> Result<(), String> {
    // BEXT-05: the browser that started the host is the source app.
    let parent = std::os::unix::process::parent_id();
    let source = wye_desktop::source_app::detect(Path::new("/proc"), parent);
    let mut service = Service::new().map_err(|error| format!("cannot start: {error}"))?;
    let mut open = |link: &Link| service.open(link, context(link, &source, parent));
    serve(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut open,
    )
    .map_err(|error| error.to_string())
}

fn with_xdg(work: impl FnOnce(&XdgDirs) -> Result<(), String>) -> Result<(), String> {
    let xdg = XdgDirs::from_env().map_err(|error| error.to_string())?;
    work(&xdg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_flags_change_the_mode_bext_04() {
        let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(mode(&args(&["--install"])), Mode::Install);
        assert_eq!(mode(&args(&["--remove"])), Mode::Remove);
        assert_eq!(mode(&args(&["chrome-extension://abc/"])), Mode::Serve);
        assert_eq!(
            mode(&args(&[
                "/home/u/.mozilla/native-messaging-hosts/x.json",
                "wye@soldunov.dev"
            ])),
            Mode::Serve
        );
        assert_eq!(mode(&[]), Mode::Serve);
    }
}
