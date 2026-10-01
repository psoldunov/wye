//! The GTK application: `adw::Application` with Wye's application ID, the
//! `GResource`, and the registry of surfaces.
//!
//! - The application ID is `dev.soldunov.wye`, so the Wayland `app_id`
//!   matches `data/applications/dev.soldunov.wye.desktop` and the Shell shows
//!   Wye's name and icon. The application is `NON_UNIQUE`: the service owns
//!   the bus name `dev.soldunov.wye`, and this host's uniqueness comes from
//!   `dev.soldunov.wye.Gtk` (`crate::host`).
//! - The resident host holds itself: closing windows never quits it;
//!   `Windows1.Quit` does.
//! - [`Host`] keeps one [`Presenter`] per surface, created on first use, and
//!   hands each [`Route`] to it (SET-04: one instance per window).
//!
//! To add a surface: a variant in `crate::surface`, a type implementing
//! [`Presenter`] in its own module, its line in [`Host::create`], and
//! `fixtures/<surface>.json`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;

use crate::cli::Scheme;
use crate::dispatch::{self, Sink, SinkClosed};
use crate::route::{Delivery, Route};
use crate::surface::Surface;
use crate::{about, history, onboarding, picker, script_editor, selftest, settings, tray_menu};

/// The application ID: the desktop entry's, for the Wayland `app_id`.
pub const APP_ID: &str = "dev.soldunov.wye";

/// Where the `GResource` lives (`data/resources.gresource.xml`); also the
/// application's resource base path, so `style.css` and `icons/` load
/// automatically.
pub const RESOURCE_BASE: &str = "/dev/soldunov/wye";

/// One window or dialog the host shows.
pub trait Presenter {
    /// Show, raise and focus the surface for `ShowWindow(key, argument)`
    /// (SET-04), creating it on first use.
    fn present(&self, key: &str, argument: &str);

    /// Act on a route: windows are only ever shown; the picker is also
    /// closed (`ClosePicker`) and the tray menu toggled (`crate::route`).
    fn act(&self, action: crate::route::Action, key: &str, argument: &str) {
        let _ = action;
        self.present(key, argument);
    }

    /// `Windows1.Quit` is coming: settle what must not be lost (a pending
    /// picker's answer, PIPE-13; unsaved edits, SCR-10), then call `quit`.
    /// Not calling it keeps the host running.
    fn before_quit(&self, quit: Box<dyn FnOnce()>) {
        quit();
    }
}

/// How the process runs.
#[derive(Debug, Clone)]
pub enum Launch {
    /// The D-Bus-activated host: wait for `Windows1` calls, after showing
    /// the window the command line named, if any.
    Resident {
        window: Option<(wye_api::actions::Window, String)>,
    },
    /// `--self-test-child`: feed one surface its fixtures, then quit.
    SelfTest {
        surface: Surface,
        snapshots: Option<PathBuf>,
        scheme: Option<Scheme>,
    },
}

/// The surfaces, created on first use.
pub struct Host {
    app: adw::Application,
    surfaces: RefCell<HashMap<Surface, Rc<dyn Presenter>>>,
    /// `--self-test-child`: cases may force a colour scheme or open menus;
    /// the resident host takes its requests as they are.
    self_test: bool,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Host")
            .field(
                "surfaces",
                &self.surfaces.borrow().keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Host {
    fn new(app: &adw::Application, self_test: bool) -> Rc<Self> {
        Rc::new(Self {
            app: app.clone(),
            surfaces: RefCell::default(),
            self_test,
        })
    }

    /// Act on a delivery from the D-Bus thread (or the self-test).
    pub fn deliver(&self, delivery: Delivery) {
        match delivery {
            Delivery::Route(route) => self.route(&route),
            Delivery::Quit => self.quit(),
        }
    }

    /// Hand `route` to its surface.
    pub fn route(&self, route: &Route) {
        if self.self_test {
            apply_scheme(&route.argument);
        }
        let presenter = self.presenter(route.surface);
        presenter.act(route.action, &route.key, &route.argument);
    }

    fn presenter(&self, surface: Surface) -> Rc<dyn Presenter> {
        let mut surfaces = self.surfaces.borrow_mut();
        Rc::clone(
            surfaces
                .entry(surface)
                .or_insert_with(|| self.create(surface)),
        )
    }

    /// `Windows1.Quit`: each surface settles first (see
    /// [`Presenter::before_quit`]), one after the other, then the
    /// application quits.
    fn quit(&self) {
        let surfaces: Vec<_> = self.surfaces.borrow().values().cloned().collect();
        quit_after(surfaces, self.app.clone());
    }

    fn create(&self, surface: Surface) -> Rc<dyn Presenter> {
        match surface {
            Surface::Settings => Rc::new(settings::Settings::new(&self.app)),
            Surface::About => Rc::new(about::About::new()),
            Surface::History => Rc::new(history::History::new(&self.app)),
            Surface::ScriptEditor => Rc::new(script_editor::ScriptEditor::new(&self.app)),
            Surface::Onboarding => Rc::new(onboarding::Onboarding::new()),
            Surface::Kit => Rc::new(selftest::kit::Kit::new(&self.app)),
            Surface::Picker => Rc::new(picker::Picker::new(&self.app, self.self_test)),
            Surface::TrayMenu => Rc::new(tray_menu::TrayMenu::new(&self.app)),
        }
    }
}

/// Let the last of `surfaces` settle, then the others, then quit `app`.
fn quit_after(mut surfaces: Vec<Rc<dyn Presenter>>, app: adw::Application) {
    match surfaces.pop() {
        Some(surface) => surface.before_quit(Box::new(move || quit_after(surfaces, app))),
        None => app.quit(),
    }
}

/// A self-test case may force a colour scheme: `{"scheme": "dark", …}`.
fn apply_scheme(argument: &str) {
    if !argument.trim_start().starts_with('{') {
        return;
    }
    let scheme = serde_json::from_str::<Value>(argument)
        .ok()
        .and_then(|value| value.get("scheme")?.as_str().and_then(Scheme::parse));
    if let Some(scheme) = scheme {
        set_scheme(scheme);
    }
}

/// Force the light or dark style.
pub fn set_scheme(scheme: Scheme) {
    let forced = match scheme {
        Scheme::Light => adw::ColorScheme::ForceLight,
        Scheme::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(forced);
}

/// Deliveries from the D-Bus thread, queued for the main context.
struct MainSink(tokio::sync::mpsc::UnboundedSender<Delivery>);

impl Sink for MainSink {
    fn deliver(&self, delivery: Delivery) -> Result<(), SinkClosed> {
        self.0.send(delivery).map_err(|_| SinkClosed)
    }
}

/// Register the `GResource` built by `build.rs`.
fn register_resources() -> anyhow::Result<()> {
    let bytes = glib::Bytes::from_static(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/wye-gtk.gresource"
    )));
    let resource = gio::Resource::from_data(&bytes)?;
    gio::resources_register(&resource);
    Ok(())
}

/// Run the application until it quits.
///
/// # Errors
///
/// When GTK cannot start (no display) or the resources cannot be loaded.
pub fn run(launch: &Launch) -> anyhow::Result<ExitCode> {
    register_resources()?;
    // Windows without an application (a parentless dialog) get the same
    // app_id from the program name.
    glib::set_prgname(Some(APP_ID));
    glib::set_application_name("Wye");
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .resource_base_path(RESOURCE_BASE)
        .build();
    let outcome: Rc<RefCell<ExitCode>> = Rc::new(RefCell::new(ExitCode::SUCCESS));
    let started = Rc::new(std::cell::Cell::new(false));
    let launch = launch.clone();
    let resident = matches!(launch, Launch::Resident { .. });
    app.connect_activate(glib::clone!(
        #[strong]
        outcome,
        move |app| {
            if started.replace(true) {
                return;
            }
            let host = Host::new(app, matches!(launch, Launch::SelfTest { .. }));
            add_actions(app, &host);
            match &launch {
                Launch::Resident { window } => {
                    attach(&host);
                    if let Some((window, argument)) = window {
                        host.route(&Route {
                            surface: Surface::for_window(*window),
                            action: crate::route::Action::Show,
                            key: window.as_str().to_owned(),
                            argument: argument.clone(),
                        });
                    }
                }
                Launch::SelfTest {
                    surface,
                    snapshots,
                    scheme,
                } => selftest::child::start(
                    app,
                    &host,
                    *surface,
                    snapshots.clone(),
                    *scheme,
                    Rc::clone(&outcome),
                ),
            }
        }
    ));
    // The resident host stays until `Quit()`; closing windows never ends it.
    let hold = resident.then(|| app.hold());
    let status = app.run_with_args::<&str>(&[]);
    drop(hold);
    if status != glib::ExitCode::SUCCESS {
        return Ok(ExitCode::FAILURE);
    }
    Ok(outcome.replace(ExitCode::SUCCESS))
}

/// App-wide actions: `app.settings` (Ctrl+, from any Wye window, SET-04).
fn add_actions(app: &adw::Application, host: &Rc<Host>) {
    let settings = gio::SimpleAction::new("settings", None);
    settings.connect_activate(glib::clone!(
        #[weak]
        host,
        move |_, _| {
            host.route(&Route {
                surface: Surface::Settings,
                action: crate::route::Action::Show,
                key: wye_api::actions::Window::Settings.as_str().to_owned(),
                argument: String::new(),
            });
        }
    ));
    app.add_action(&settings);
    app.set_accels_for_action("app.settings", &["<Control>comma"]);
}

/// Take the deliveries the D-Bus object queued, and every later one.
fn attach(host: &Rc<Host>) {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let host = Rc::clone(host);
    glib::spawn_future_local(async move {
        while let Some(delivery) = receiver.recv().await {
            host.deliver(delivery);
        }
    });
    if let Err(error) = dispatch::global().attach(Box::new(MainSink(sender))) {
        tracing::warn!(%error, "cannot take the queued window requests");
    }
}
