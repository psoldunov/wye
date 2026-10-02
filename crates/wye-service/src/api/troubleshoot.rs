//! `GetTroubleshooting`: the About window's troubleshooting text
//! (DLG-ABT-02), built from `Status`, the inventory, the session, the tray
//! and the picker frontend.

use std::fmt::Write as _;

use wye_api::status::{Capabilities, Status};

use super::Result;
use crate::context::ServiceContext;

/// What a mechanism of `None` reads as.
const UNAVAILABLE: &str = "unavailable";
/// Why the clipboard switches are off (19-help-texts.md, "Clipboard
/// features").
const CLIPBOARD_HELP: &str = "This session does not let apps watch the clipboard. On GNOME, \
                              enable Wye's Shell integration.";

/// `dev.soldunov.wye1.GetTroubleshooting`: plain text, one fact per line.
pub async fn get_troubleshooting(ctx: &ServiceContext) -> Result<String> {
    let status = super::state::status(ctx).await;
    let desktops = ctx
        .environment()
        .map(|environment| environment.xdg.current_desktops.join(":"))
        .unwrap_or_default();
    let session = Session {
        desktops,
        kind: session_type(|name| std::env::var(name).ok()),
        tray: super::tray::mechanism(ctx).await,
        picker: super::picker::host::describe(ctx).await,
    };
    let apps = match super::inventory::current(ctx).await {
        Ok(scan) => {
            let handlers = scan.inventory.web_handlers();
            let profiles: usize = handlers.iter().map(|app| app.profiles.len()).sum();
            format!(
                "{} apps, {} handle web links, {profiles} browser profiles",
                scan.inventory.apps().count(),
                handlers.len()
            )
        }
        Err(error) => format!("not scanned ({error})"),
    };
    Ok(text(&status, &session, &apps))
}

/// What the report says about this session besides `Status`.
#[derive(Debug, Default)]
struct Session {
    /// `XDG_CURRENT_DESKTOP`, joined with `:`.
    desktops: String,
    /// `wayland`, `x11` or what `XDG_SESSION_TYPE` says.
    kind: String,
    /// How the tray icon shows.
    tray: String,
    /// The picker frontend and the host serving it.
    picker: String,
}

/// The session type: `XDG_SESSION_TYPE`, else guessed from the display
/// variables the service sees.
fn session_type(var: impl Fn(&str) -> Option<String>) -> String {
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    set("XDG_SESSION_TYPE")
        .or_else(|| set("WAYLAND_DISPLAY").map(|_| "wayland".to_owned()))
        .or_else(|| set("DISPLAY").map(|_| "x11".to_owned()))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The report.
fn text(status: &Status, session: &Session, apps: &str) -> String {
    let mut out = String::new();
    let mut line = |label: &str, value: &str| {
        // Writing to a String cannot fail.
        let _ = writeln!(out, "{label}: {value}");
    };
    line("Wye", env!("CARGO_PKG_VERSION"));
    line(
        "Desktop",
        if session.desktops.is_empty() {
            "unknown"
        } else {
            &session.desktops
        },
    );
    line("Session type", &session.kind);
    line("Default browser", &default_browser(status));
    let config = &status.config;
    line("Configuration", &config.path);
    line(
        "Configuration writable",
        yes_no(config.writable && config.error.is_none()),
    );
    line("Configuration lossless", yes_no(config.lossless));
    if let Some(error) = &config.error {
        line("Configuration error", error);
    }
    for warning in &config.warnings {
        line("Configuration warning", warning);
    }
    line("Apps", apps);
    line("Tray", &session.tray);
    line("Picker frontend", &session.picker);
    capabilities(&status.capabilities, &mut line);
    line("Screen locked", yes_no(status.locked));
    line("Onboarding done", yes_no(status.ui_state.onboarding_done));
    out
}

fn default_browser(status: &Status) -> String {
    let registration = &status.default_browser;
    let now = if registration.is_default {
        "Wye".to_owned()
    } else {
        registration.current.as_ref().map_or_else(
            || "none".to_owned(),
            |app| format!("{} ({})", app.name, app.id),
        )
    };
    match &registration.previous {
        Some(previous) => format!("{now}; before Wye: {} ({})", previous.name, previous.id),
        None => now,
    }
}

fn capabilities(capabilities: &Capabilities, line: &mut impl FnMut(&str, &str)) {
    let mechanism =
        |value: &Option<String>| value.clone().unwrap_or_else(|| UNAVAILABLE.to_owned());
    line("Held keys", &mechanism(&capabilities.held_keys));
    line("Pointer position", &mechanism(&capabilities.pointer));
    let fallbacks = if capabilities.source_app_fallbacks.is_empty() {
        UNAVAILABLE.to_owned()
    } else {
        capabilities.source_app_fallbacks.join(", ")
    };
    line("Source app fallbacks", &fallbacks);
    line(
        "Clipboard reading",
        &mechanism(&capabilities.clipboard_read),
    );
    line(
        "Clipboard watching",
        &mechanism(&capabilities.clipboard_watch),
    );
    line(
        "Clipboard writing",
        &mechanism(&capabilities.clipboard_write),
    );
    if capabilities.clipboard_watch.is_none() {
        line("Clipboard features", CLIPBOARD_HELP);
    }
    line(
        "Global shortcuts",
        &mechanism(&capabilities.global_shortcuts),
    );
    line("Lock detection", &mechanism(&capabilities.lock_detection));
    line("Layer shell", yes_no(capabilities.layer_shell));
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_is_listed() {
        let status = Status {
            capabilities: Capabilities {
                held_keys: Some("wayland-layer-shell".to_owned()),
                ..Capabilities::default()
            },
            ..Status::default()
        };
        let session = Session {
            desktops: "KDE".to_owned(),
            kind: "wayland".to_owned(),
            tray: "status-notifier-item".to_owned(),
            picker: "auto (Qt); Qt running".to_owned(),
        };
        let report = text(&status, &session, "3 apps");
        for label in [
            "Held keys: wayland-layer-shell",
            "Pointer position: unavailable",
            "Source app fallbacks",
            "Clipboard reading",
            "Clipboard watching",
            "Clipboard writing",
            "Clipboard features: This session does not let apps watch the clipboard.",
            "Global shortcuts",
            "Lock detection",
            "Layer shell: no",
            "Desktop: KDE",
            "Session type: wayland",
            "Tray: status-notifier-item",
            "Picker frontend: auto (Qt); Qt running",
            "Default browser: none",
        ] {
            assert!(report.contains(label), "{label} missing:\n{report}");
        }
    }

    #[test]
    fn dlg_abt_02_the_session_type_falls_back_to_the_display() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        assert_eq!(session_type(vars(&[("XDG_SESSION_TYPE", "x11")])), "x11");
        assert_eq!(
            session_type(vars(&[
                ("XDG_SESSION_TYPE", ""),
                ("WAYLAND_DISPLAY", "wayland-0")
            ])),
            "wayland"
        );
        assert_eq!(session_type(vars(&[("DISPLAY", ":0")])), "x11");
        assert_eq!(session_type(vars(&[])), "unknown");
    }
}
