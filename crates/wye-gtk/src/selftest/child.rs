//! One self-test surface, in this process (`wye-gtk --self-test-child
//! SURFACE`): show every case of its fixtures through the same
//! [`Host::route`] a D-Bus call takes, let each render, optionally save
//! snapshots, then print the pass line and quit. `GLib`'s messages go to
//! stderr as marked lines (`super::log`), which the parent reads.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use super::log::{self, Level, PASS_LINE};
use super::{fixtures, snapshot};
use crate::app::{self, Host};
use crate::cli::Scheme;
use crate::surface::Surface;

/// How long a case gets to lay out and draw before the next one.
const SETTLE: Duration = Duration::from_millis(300);

/// How long a case gets before its snapshot: long enough for a dialog's
/// opening animation to end.
const SETTLE_FOR_SNAPSHOT: Duration = Duration::from_millis(900);

/// The font the GNOME session uses; the private X server has no settings
/// daemon, so the self-test sets it like GNOME would (fontconfig falls back
/// when it is not installed).
const GNOME_FONT: &str = "Adwaita Sans 11";

/// GNOME's title buttons: close only.
const GNOME_DECORATION_LAYOUT: &str = "appmenu:close";

/// The log domain of the self-test's own lines.
const DOMAIN: &str = "wye-gtk";

/// Run the child.
///
/// # Errors
///
/// When GTK cannot start.
pub fn run(
    surface: Surface,
    snapshots: Option<PathBuf>,
    scheme: Option<Scheme>,
) -> anyhow::Result<ExitCode> {
    log::install_writer();
    app::run(&app::Launch::SelfTest {
        surface,
        snapshots,
        scheme,
    })
}

/// Start the cases once the application is active; `outcome` gets the exit
/// code before the application quits.
pub fn start(
    app: &adw::Application,
    host: &Rc<Host>,
    surface: Surface,
    snapshots: Option<PathBuf>,
    scheme: Option<Scheme>,
    outcome: Rc<RefCell<ExitCode>>,
) {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_font_name(Some(GNOME_FONT));
        settings.set_gtk_decoration_layout(Some(GNOME_DECORATION_LAYOUT));
    }
    if let Some(scheme) = scheme {
        app::set_scheme(scheme);
    }
    let app = app.clone();
    let host = Rc::clone(host);
    // Keep the application alive while the cases run between windows.
    let hold = app.hold();
    glib::MainContext::default().spawn_local(async move {
        let code = match play(&host, surface, snapshots).await {
            Ok(()) => {
                log::print(
                    Level::Message,
                    DOMAIN,
                    &format!("{PASS_LINE} {}", surface.name()),
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                log::print(Level::Critical, DOMAIN, &format!("{error:#}"));
                ExitCode::FAILURE
            }
        };
        outcome.replace(code);
        drop(hold);
        app.quit();
    });
}

async fn play(host: &Host, surface: Surface, snapshots: Option<PathBuf>) -> anyhow::Result<()> {
    let cases = fixtures::cases(surface)?;
    for (index, case) in cases.iter().enumerate() {
        host.route(&case.route(surface));
        let settle = if snapshots.is_some() {
            SETTLE_FOR_SNAPSHOT
        } else {
            SETTLE
        };
        glib::timeout_future(settle).await;
        if let Some(dir) = &snapshots {
            let prefix = snapshot::prefix(dir, surface, index, case);
            for path in snapshot::save_windows(&prefix)? {
                println!("{path}");
            }
        }
    }
    Ok(())
}
