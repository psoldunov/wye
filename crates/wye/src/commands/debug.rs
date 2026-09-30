//! `wye debug …`: diagnostics for what only a real session can show.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use wye_desktop::XdgDirs;
use wye_service::platform::modifiers::HeldKeys;
use wye_service::platform::probe::{self, Probed, SessionProbe};

use super::Console;
use crate::cli::DebugAction;
use crate::notice;
use crate::paths::Paths;

/// What an unknown answer prints as.
const UNKNOWN: &str = "unknown";

/// Run one diagnostic.
pub fn run(console: &mut Console<'_>, action: &DebugAction) -> ExitCode {
    match action {
        DebugAction::Probe { delay } => run_probe(console, delay.unwrap_or(0)),
    }
}

/// `wye debug probe`: the session probes as the service runs them for a
/// link (KEY-06, PICK-02, source-app step 4), in this process.
fn run_probe(console: &mut Console<'_>, delay: u64) -> ExitCode {
    if delay > 0 {
        notice::write(
            console.err,
            format_args!(
                "wye: probing in {delay} s: hold the keys to test and focus the window to read"
            ),
        );
        std::thread::sleep(Duration::from_secs(delay));
    }
    let config = config_path();
    let probed = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(anyhow::Error::from)
        .and_then(|runtime| runtime.block_on(probe::run(config.as_deref())));
    match probed {
        Ok(probed) => {
            if let Err(error) = console.out.write_all(report(&probed).as_bytes()) {
                notice::write(console.err, format_args!("wye: {error}"));
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            notice::write(console.err, format_args!("wye: debug probe: {error:#}"));
            ExitCode::FAILURE
        }
    }
}

/// `config.toml`, for `advanced.held-keys`; `None` without a home
/// directory.
fn config_path() -> Option<PathBuf> {
    XdgDirs::from_env()
        .ok()
        .map(|xdg| Paths::from_env(&xdg).config)
}

/// The probe's answers, one line each.
fn report(probed: &SessionProbe) -> String {
    let held = probed.modifiers.value.as_ref().map(|held| {
        if held.is_empty() {
            "none".to_owned()
        } else {
            held.iter()
                .map(|modifier| modifier.as_str())
                .collect::<Vec<_>>()
                .join("+")
        }
    });
    let pointer = probed
        .pointer
        .value
        .as_ref()
        .map(|at| format!("{},{} on {}", at.x, at.y, at.output));
    let focus = probed.focus.value.as_ref().map(|app| {
        let pid = app
            .pid
            .map_or_else(|| "?".to_owned(), |pid| pid.to_string());
        format!(
            "pid {pid}, desktop file {}, class {}",
            app.desktop_id.as_deref().unwrap_or("?"),
            app.resource_class.as_deref().unwrap_or("?"),
        )
    });
    let setting = match probed.held_keys {
        HeldKeys::Auto => "auto",
        HeldKeys::Off => "off",
    };
    let mut text = String::new();
    line(&mut text, "held modifiers", held, &probed.modifiers);
    line(&mut text, "pointer", pointer, &probed.pointer);
    line(&mut text, "focused app", focus, &probed.focus);
    // Writing to a String cannot fail.
    let _ = writeln!(text, "{:<15} {setting}", "held-keys:");
    text
}

fn line<T>(text: &mut String, label: &str, value: Option<String>, probed: &Probed<T>) {
    let value = value.unwrap_or_else(|| UNKNOWN.to_owned());
    let how = probed.mechanism.map_or_else(
        || "no mechanism in this session".to_owned(),
        |mechanism| format!("{mechanism}, {} ms", probed.elapsed.as_millis()),
    );
    // Writing to a String cannot fail.
    let _ = writeln!(text, "{:<15} {value}  ({how})", format!("{label}:"));
}

#[cfg(test)]
mod tests {
    use wye_api::context::Modifier;
    use wye_api::picker::Placement;
    use wye_service::platform::FocusedApp;

    use super::*;

    fn probed<T>(mechanism: Option<&'static str>, value: Option<T>) -> Probed<T> {
        Probed {
            mechanism,
            value,
            elapsed: Duration::from_millis(12),
        }
    }

    #[test]
    fn every_answer_gets_a_line() {
        let probed = SessionProbe {
            held_keys: HeldKeys::Auto,
            modifiers: probed(
                Some("wayland-layer-shell"),
                Some(vec![Modifier::Shift, Modifier::Ctrl]),
            ),
            pointer: probed(
                Some("kwin-script"),
                Some(Placement {
                    output: "DP-1".to_owned(),
                    x: 451,
                    y: 306,
                }),
            ),
            focus: probed(
                Some("kwin-script"),
                Some(FocusedApp {
                    pid: Some(42),
                    desktop_id: Some("org.kde.dolphin".to_owned()),
                    resource_class: None,
                }),
            ),
        };
        assert_eq!(
            report(&probed),
            "held modifiers: Shift+Ctrl  (wayland-layer-shell, 12 ms)\n\
             pointer:        451,306 on DP-1  (kwin-script, 12 ms)\n\
             focused app:    pid 42, desktop file org.kde.dolphin, class ?  (kwin-script, 12 ms)\n\
             held-keys:      auto\n"
        );
    }

    #[test]
    fn unknown_answers_say_so() {
        let probed = SessionProbe {
            held_keys: HeldKeys::Off,
            modifiers: probed(None, None),
            pointer: probed(Some("x11"), None),
            focus: probed(None, None),
        };
        let text = report(&probed);
        assert!(text.starts_with("held modifiers: unknown  (no mechanism in this session)\n"));
        assert!(text.contains("pointer:        unknown  (x11, 12 ms)\n"));
        assert!(text.ends_with("held-keys:      off\n"));
    }

    #[test]
    fn nothing_held_is_not_unknown() {
        let probed = SessionProbe {
            held_keys: HeldKeys::Auto,
            modifiers: probed(Some("x11"), Some(Vec::new())),
            pointer: probed(None, None),
            focus: probed(None, None),
        };
        assert!(report(&probed).starts_with("held modifiers: none  (x11, 12 ms)\n"));
    }
}
