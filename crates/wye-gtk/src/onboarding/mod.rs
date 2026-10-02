//! First run (18-onboarding.md, ONB-01 to ONB-06): a small dialog that
//! walks through what Wye needs before it is useful: welcome, make Wye the
//! default browser, choose browsers, launch at login and GNOME Shell
//! integration, and the optional browser extension. It opens on first start
//! and later from the tray's **Set Up Wye…**, at the welcome step each time.
//!
//! ONB-06 on GNOME: an `AdwNavigationView` in an `AdwDialog`. The tray opens
//! it with no window to belong to, so it is presented without a parent, as
//! About is, and libadwaita gives it a window of its own. **Back** is on
//! every step after the first (Alt+Left and Escape go back too, the
//! navigation view's own keys), progress dots are at the bottom, and closing
//! the dialog early counts as done. Every choice is stored as it is made.
//!
//! GTK-free logic from wye-ui (one source for both frontends; crates/wye-ui
//! owns it): [`flow`], [`choices`], [`desktop`], [`view`], [`sync`];
//! `choices`, `sync` and `fixtures` are symlinks, so their nested `tests`
//! modules and `fixtures/onboarding.json` resolve inside this crate.
//!
//! On GTK: [`pages`], [`browsers`], [`integration`] (the steps), [`footer`],
//! [`shell`] (GNOME Shell's extension, ONB-04).
//!
//! The self-test's argument is a JSON object: `fixture` (what the service
//! would return, `settings::fixture::Fixture`), `scheme`, `step`, `desktop`
//! (`kde`, `gnome`, `other`) and `shell` (`missing`, `disabled`, `enabled`).
//!
//! KDE counterpart: crates/wye-ui/qml/onboarding/, crates/wye-ui/src/onboarding/.

mod browsers;
pub mod choices;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/onboarding/desktop.rs"]
pub mod desktop;
#[cfg(test)]
mod fixtures;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/onboarding/flow.rs"]
pub mod flow;
mod footer;
mod integration;
mod pages;
mod shell;
pub mod sync;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/onboarding/view.rs"]
pub mod view;

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;
use serde::Deserialize;
use serde_json::Value;
use wye_api::Error;

use crate::app::Presenter;
use crate::service;
use crate::settings::fixture::Fixture;
use crate::settings::snapshot::Snapshot;
use desktop::Desktop;
use flow::{Effect, Flow};
use footer::Footer;
use pages::{Context, Intent, Steps};
use shell::ShellState;
use sync::Action;
use view::View;

/// ONB-06: the dialog's size; the browsers step's list scrolls inside.
const SIZE: (i32, i32) = (560, 600);

/// What a `ShowWindow("first-run", …)` argument may hold (the self-test's).
#[derive(Debug, Default, Deserialize)]
struct Request {
    fixture: Option<Value>,
    step: Option<usize>,
    desktop: Option<String>,
    shell: Option<String>,
}

impl Request {
    fn parse(argument: &str) -> Self {
        if !argument.trim_start().starts_with('{') {
            return Self::default();
        }
        serde_json::from_str(argument)
            .inspect_err(|error| tracing::warn!(%error, "first run: the argument is not a request"))
            .unwrap_or_default()
    }
}

/// The first-run surface.
#[derive(Debug, Default)]
pub struct Onboarding {
    controller: OnceCell<Rc<Controller>>,
}

impl Onboarding {
    /// The surface; the dialog is built when first shown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Presenter for Onboarding {
    fn present(&self, _key: &str, argument: &str) {
        self.controller
            .get_or_init(Controller::new)
            .show(&Request::parse(argument));
    }
}

/// The dialog and the walk-through's state.
struct Controller {
    dialog: adw::Dialog,
    navigation: adw::NavigationView,
    banner: adw::Banner,
    steps: Steps,
    footer: Footer,
    me: Weak<Self>,
    flow: Cell<Flow>,
    snapshot: RefCell<Snapshot>,
    desktop: Cell<Desktop>,
    shell: Cell<ShellState>,
    /// A fixture stands in for the service (self-test).
    offline: Cell<bool>,
    /// Calls in flight; an answer that arrives meanwhile would undo the
    /// choice already shown.
    pending: Cell<u32>,
    finished: Cell<bool>,
    /// Set while code moves the navigation view, so its signals are not
    /// taken as the user's.
    navigating: Cell<bool>,
}

impl std::fmt::Debug for Controller {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Controller")
            .field("flow", &self.flow.get())
            .field("offline", &self.offline.get())
            .finish_non_exhaustive()
    }
}

impl Controller {
    fn new() -> Rc<Self> {
        let controller = Rc::new_cyclic(|me: &Weak<Self>| {
            let weak = me.clone();
            let send: pages::Send = Rc::new(move |intent| {
                if let Some(controller) = weak.upgrade() {
                    controller.on_intent(intent);
                }
            });
            let steps = Steps::new(&send);
            let navigation = adw::NavigationView::new();
            let footer = Footer::new();
            let banner = adw::Banner::builder().button_label("Dismiss").build();
            banner.connect_button_clicked(|banner| banner.set_revealed(false));
            let dialog = frame(&navigation, &banner, &footer);
            Self {
                dialog,
                navigation,
                banner,
                steps,
                footer,
                me: me.clone(),
                flow: Cell::new(Flow::new()),
                snapshot: RefCell::default(),
                desktop: Cell::new(Desktop::detect()),
                shell: Cell::new(ShellState::Unknown),
                offline: Cell::new(false),
                pending: Cell::new(0),
                finished: Cell::new(false),
                navigating: Cell::new(false),
            }
        });
        controller.connect();
        controller
    }

    /// Run `then` with the controller, if it still exists.
    fn with(&self, then: impl Fn(&Self) + 'static) -> impl Fn() + 'static {
        let me = self.me.clone();
        move || {
            if let Some(controller) = me.upgrade() {
                then(&controller);
            }
        }
    }

    fn connect(&self) {
        let back = self.with(Self::back);
        self.footer.back.connect_clicked(move |_| back());
        let next = self.with(Self::next);
        self.footer.next.connect_clicked(move |_| next());
        let skip = self.with(Self::skip_default);
        self.footer.skip.connect_clicked(move |_| skip());
        // ONB-06: closing early counts as done.
        let finish = self.with(Self::finish);
        self.dialog.connect_closed(move |_| finish());
        // The navigation view's own Back (Alt+Left, Escape, a swipe).
        let popped = self.with(Self::follow_navigation);
        self.navigation.connect_popped(move |_, _| popped());
    }

    /// ONB-01: every opening starts at the welcome step.
    fn show(&self, request: &Request) {
        if let Some(fixture) = &request.fixture {
            self.load_fixture(fixture);
        }
        if let Some(desktop) = &request.desktop {
            self.desktop.set(Desktop::of(desktop));
        }
        if let Some(shell) = request.shell.as_deref().and_then(ShellState::parse) {
            self.shell.set(shell);
        }
        self.flow.set(Flow::new());
        self.finished.set(false);
        if let Some(step) = request.step {
            self.flow.set(self.flow.get().at(step).0);
        }
        self.publish();
        self.reset_navigation();
        if !self.offline.get() {
            self.reload();
            self.query_shell();
        }
        self.dialog.present(None::<&gtk::Widget>);
    }

    fn load_fixture(&self, fixture: &Value) {
        match Fixture::parse(&fixture.to_string()) {
            Ok(fixture) => {
                self.offline.set(true);
                self.banner.set_revealed(false);
                let next = fixture.apply(&self.snapshot.borrow());
                self.snapshot.replace(next);
            }
            Err(error) => {
                glib::g_critical!(
                    "wye-gtk",
                    "first run: the fixture is not service data: {error}"
                );
            }
        }
    }

    /// Show the step and the data on every page.
    fn publish(&self) {
        let snapshot = self.snapshot.borrow();
        let view = View::build(self.flow.get(), &snapshot, self.desktop.get());
        let current_icon = snapshot
            .status
            .default_browser
            .current
            .as_ref()
            .and_then(|app| app.icon.as_deref());
        self.steps.show(&Context {
            view: &view,
            targets: &snapshot.targets,
            current_icon,
            desktop: self.desktop.get(),
            shell: self.shell.get(),
        });
        self.footer.show(&view);
    }

    /// The navigation view holds the pages up to the current step.
    fn reset_navigation(&self) {
        let step = self.flow.get().step();
        self.navigating.set(true);
        self.navigation.replace(&self.steps.through(step));
        self.navigating.set(false);
    }

    /// The user went back with the navigation view's own keys.
    fn follow_navigation(&self) {
        if self.navigating.get() {
            return;
        }
        let shown = self.navigation.visible_page();
        let step = shown.and_then(|page| self.steps.step_of(&page));
        if let Some(step) = step {
            self.flow.set(self.flow.get().at(step.index()).0);
            self.publish();
        }
    }

    fn move_to(&self, flow: Flow, effect: Effect) {
        let from = self.flow.replace(flow).step();
        let to = flow.step();
        self.publish();
        self.navigating.set(true);
        if to.index() == from.index() + 1 {
            self.navigation.push(self.steps.page(to));
        } else if to.index() + 1 == from.index() {
            self.navigation.pop();
        } else if to != from {
            self.navigation.replace(&self.steps.through(to));
        }
        self.navigating.set(false);
        self.arrive(effect);
    }

    fn next(&self) {
        let (flow, effect) = self.flow.get().next();
        self.move_to(flow, effect);
    }

    fn back(&self) {
        self.move_to(self.flow.get().back(), Effect::None);
    }

    /// ONB-02, ONB-11: **Skip** keeps another app as the default and stops
    /// asking.
    fn skip_default(&self) {
        if !self.snapshot.borrow().status.default_browser.is_default {
            self.apply(Action::KeepCurrentDefault);
        }
        self.next();
    }

    fn arrive(&self, effect: Effect) {
        match effect {
            Effect::None => {}
            Effect::SeedShownBrowsers => {
                let patch = {
                    let snapshot = self.snapshot.borrow();
                    let foreign = choices::foreign_app_ids(&snapshot.services.services);
                    choices::seed_patch(&snapshot.config, &snapshot.targets, &foreign)
                };
                if let Some(patch) = patch {
                    self.apply(Action::Patch(patch));
                }
            }
            Effect::Finish => self.finish(),
        }
    }

    /// ONB-05, ONB-06: the walk-through is over (Done, or the dialog was
    /// closed): `onboardingDone`, then the dialog goes.
    fn finish(&self) {
        if self.finished.replace(true) {
            return;
        }
        self.apply(Action::Finish);
        self.dialog.close();
    }

    fn on_intent(&self, intent: Intent) {
        match intent {
            Intent::MakeDefault => self.apply(Action::MakeDefault),
            Intent::SetPrimary(target) => {
                self.apply(Action::Patch(choices::primary_patch(&target)));
            }
            Intent::Toggle(key, checked) => self.toggle(&key, checked),
            Intent::Move(from, to) => self.reorder(from, to),
            Intent::LaunchAtLogin(on) => {
                // GEN-01: the Nix configuration owns login start; the key
                // would change nothing, so it is never written.
                if !self.snapshot.borrow().status.login_managed {
                    self.apply(Action::Patch(choices::launch_patch(on)));
                }
            }
            Intent::EnableShell => self.enable_shell(),
            Intent::OpenLink(url) => self.open_link(url),
        }
    }

    /// The shown list as the checklist shows it.
    fn shown(&self) -> Vec<Value> {
        let snapshot = self.snapshot.borrow();
        let foreign = choices::foreign_app_ids(&snapshot.services.services);
        choices::effective_shown(&snapshot.config, &snapshot.targets, &foreign)
    }

    fn toggle(&self, key: &str, checked: bool) {
        match serde_json::from_str::<Value>(key) {
            Ok(target) => {
                let next = choices::toggled(&self.shown(), &target, checked);
                self.apply(Action::Patch(choices::shown_patch(&next)));
            }
            Err(error) => self.show_error(&crate::error_text::ErrorText::plain(error.to_string())),
        }
    }

    fn reorder(&self, from: usize, to: usize) {
        let checked: Vec<Value> = {
            let snapshot = self.snapshot.borrow();
            View::build(self.flow.get(), &snapshot, self.desktop.get())
                .checklist
                .into_iter()
                .filter(|row| row.checked)
                .map(|row| row.target)
                .collect()
        };
        let next = browsers::moved(&self.shown(), &checked, from, to);
        self.apply(Action::Patch(choices::shown_patch(&next)));
    }

    /// Carry out `action`, showing its effect at once.
    fn apply(&self, action: Action) {
        let offline = self.offline.get();
        let before = self.snapshot.borrow().clone();
        // A status the service has not confirmed is only shown when there
        // is no service to ask.
        let instant = offline || matches!(action, Action::Patch(_) | Action::Finish);
        if instant {
            self.snapshot.replace(sync::preview(&before, &action));
            self.publish();
        }
        if offline {
            return;
        }
        self.pending.set(self.pending.get() + 1);
        let me = self.me.clone();
        service::request(
            move |proxy| async move { sync::run(&proxy, action, &before).await },
            move |result: Result<Snapshot, Error>| {
                if let Some(controller) = me.upgrade() {
                    controller
                        .pending
                        .set(controller.pending.get().saturating_sub(1));
                    if let Err(error) = &result {
                        // Show what the service holds, not the refused choice.
                        controller.show_error(&crate::error_text::describe(error));
                        controller.reload();
                    }
                    controller.received(result.ok());
                }
            },
        );
    }

    /// Read everything from the service.
    fn reload(&self) {
        self.pending.set(self.pending.get() + 1);
        let me = self.me.clone();
        service::request(
            |proxy| async move { sync::load(&proxy).await },
            move |result: Result<Snapshot, Error>| {
                if let Some(controller) = me.upgrade() {
                    controller
                        .pending
                        .set(controller.pending.get().saturating_sub(1));
                    match result {
                        Ok(snapshot) => controller.received(Some(snapshot)),
                        Err(error) => {
                            tracing::warn!(%error, "first run: cannot read the service");
                            controller.show_error(&crate::error_text::describe(&error));
                        }
                    }
                }
            },
        );
    }

    /// A snapshot from the service; dropped while another call is in flight.
    fn received(&self, snapshot: Option<Snapshot>) {
        if self.offline.get() || self.pending.get() > 0 {
            return;
        }
        if let Some(snapshot) = snapshot {
            self.snapshot.replace(snapshot);
            self.publish();
        }
    }

    fn show_error(&self, text: &crate::error_text::ErrorText) {
        self.banner.set_title(&text.sentence());
        self.banner.set_revealed(!text.is_empty());
    }

    /// ONB-04: ask GNOME Shell about Wye's extension.
    fn query_shell(&self) {
        if self.desktop.get() != Desktop::Gnome {
            return;
        }
        self.shell_call(false);
    }

    /// ONB-04: **Enable**.
    fn enable_shell(&self) {
        if self.offline.get() {
            self.shell.set(ShellState::Enabled);
            self.publish();
            return;
        }
        self.shell_call(true);
    }

    fn shell_call(&self, enable: bool) {
        let me = self.me.clone();
        service::request(
            move |proxy| async move {
                let connection = proxy.inner().connection().clone();
                if enable {
                    shell::enable(connection).await
                } else {
                    shell::query(connection).await
                }
            },
            move |result: Result<ShellState, Error>| {
                let Some(controller) = me.upgrade() else {
                    return;
                };
                match result {
                    Ok(state) => controller.shell.set(state),
                    Err(error) if enable => {
                        controller.show_error(&crate::error_text::describe(&error));
                    }
                    Err(error) => {
                        tracing::info!(%error, "GNOME Shell did not describe Wye's extension");
                        controller.shell.set(ShellState::Unknown);
                    }
                }
                controller.publish();
            },
        );
    }

    /// ONB-05: the install page opens through Wye (BLK-17).
    fn open_link(&self, url: &str) {
        if self.offline.get() {
            tracing::info!(%url, "a link was activated in the self-test");
            return;
        }
        let me = self.me.clone();
        crate::links::open(url, move |error| {
            if let Some(controller) = me.upgrade() {
                controller.show_error(&crate::error_text::describe(&error));
            }
        });
    }
}

/// The dialog: a header bar without a title (each step has its own
/// heading), the error banner, the steps and the footer. Ctrl+W closes it
/// (SET-07); Escape goes back, then closes.
fn frame(navigation: &adw::NavigationView, banner: &adw::Banner, footer: &Footer) -> adw::Dialog {
    let view = adw::ToolbarView::builder().content(navigation).build();
    view.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    view.add_top_bar(banner);
    view.add_bottom_bar(&footer.bar);
    let dialog = adw::Dialog::builder()
        .title("Welcome to Wye")
        .content_width(SIZE.0)
        .content_height(SIZE.1)
        .child(&view)
        .build();
    dialog.add_css_class("wye-onboarding");
    let close = gtk::CallbackAction::new(|widget, _| {
        if let Some(dialog) = widget.downcast_ref::<adw::Dialog>() {
            dialog.close();
        }
        glib::Propagation::Stop
    });
    let controller = gtk::ShortcutController::new();
    controller.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control>w"),
        Some(close),
    ));
    dialog.add_controller(controller);
    dialog
}
