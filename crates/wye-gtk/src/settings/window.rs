//! The Settings window (03-settings-window.md): one window with seven pages,
//! titled with the current page (SET-01); a page switcher in the header bar
//! with each page's icon above its label and the selected one in the accent
//! colour (SET-02, SET-03, `data/style.css`), moving to a bar at the bottom
//! when the window is too narrow for it; one size for every page with the
//! page scrolling inside (SET-05); instant apply (SET-06); Escape and Ctrl+W
//! to close, which hides it and keeps Wye running (SET-07, KEY-50); the last
//! page reopened (SET-08).
//!
//! Banners under the header bar say when the configuration file is read-only
//! (SET-06), when a change failed (with Dismiss), and when the service cannot
//! be read.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/SettingsWindow.qml.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};

use super::pages::{self, Context, Page, PageInfo};
use super::request::Request;
use super::sheets;
use super::store::SettingsStore;
use super::sync::Action;
use crate::widgets;

/// SET-05: the default size, in logical pixels. KDE's is 34 × 38 grid units
/// (about 780 × 720 at the default font); this is the same, which also
/// fits the seven switcher items with room to spare.
const DEFAULT_SIZE: (i32, i32) = (780, 720);

/// SET-05: the smallest size, 26 × 20 grid units.
const MINIMUM_SIZE: (i32, i32) = (468, 360);

/// SET-05: the default height is at most this share of the screen's height.
const MAX_SCREEN_SHARE: f64 = 0.85;

/// Below this width the switcher moves from the header bar to a bar at the
/// bottom (libadwaita's adaptive pattern), so the seven items never squeeze.
const NARROW: &str = "max-width: 640sp";

/// SET-06: why nothing can be saved. Shorter than KDE's inline message: a
/// banner is one centred line where it fits.
const READ_ONLY: &str = "The configuration file is read-only. Change it where it is managed, such as Nix or home-manager.";

/// The window, its pages and its store.
pub struct SettingsWindow {
    window: adw::ApplicationWindow,
    stack: adw::ViewStack,
    store: SettingsStore,
    pages: Vec<(&'static PageInfo, Box<dyn Page>)>,
    /// Set while code switches the page, so it is not remembered as the
    /// user's choice.
    selecting: Cell<bool>,
    /// The user (or a request) chose a page since the window was asked for:
    /// the last page, once known, must not replace it (SET-08).
    navigated: Cell<bool>,
}

impl std::fmt::Debug for SettingsWindow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsWindow")
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl SettingsWindow {
    /// Build the window for `app`; it is shown by [`Self::handle`].
    #[must_use]
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let store = SettingsStore::new();
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("General")
            .default_width(DEFAULT_SIZE.0)
            .default_height(default_height(None))
            .width_request(MINIMUM_SIZE.0)
            .height_request(MINIMUM_SIZE.1)
            .hide_on_close(true)
            .build();
        // Added, not set: the builder's `css_classes` would replace the
        // window's own `background` class.
        window.add_css_class("wye-settings");
        // SET-05: once the window knows its monitor, size it for that one,
        // before it first shows.
        window.connect_realize(|window| {
            if !window.is_maximized() {
                window.set_default_height(default_height(Some(window)));
            }
        });
        let stack = adw::ViewStack::new();
        let context = Context::new(store.clone(), &window);
        let pages: Vec<(&'static PageInfo, Box<dyn Page>)> = pages::PAGES
            .iter()
            .map(|info| (info, pages::build(info, &context)))
            .collect();
        for (info, page) in &pages {
            widgets::page::widen(page.widget());
            stack.add_titled_with_icon(page.widget(), Some(info.id), info.title, info.icon);
        }
        let this = Rc::new(Self {
            window,
            stack,
            store,
            pages,
            selecting: Cell::new(false),
            navigated: Cell::new(false),
        });
        this.build_frame();
        this.connect_signals();
        this
    }

    /// Header bar with the switcher, banners, pages, the narrow-window bar.
    fn build_frame(&self) {
        let switcher = adw::ViewSwitcher::builder()
            .stack(&self.stack)
            .policy(adw::ViewSwitcherPolicy::Narrow)
            .build();
        let header = adw::HeaderBar::builder().title_widget(&switcher).build();
        let bar = adw::ViewSwitcherBar::builder().stack(&self.stack).build();
        let view = adw::ToolbarView::builder()
            .content(&self.stack)
            .top_bar_style(adw::ToolbarStyle::Raised)
            .build();
        view.add_top_bar(&header);
        for banner in self.banners() {
            view.add_top_bar(&banner);
        }
        view.add_bottom_bar(&bar);
        // Toasts (RUL-06's Undo) show over the pages; built here, so showing
        // the first one never moves the window's content (and its focus).
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&view));
        self.window.set_content(Some(&toasts));

        let narrow =
            adw::Breakpoint::new(adw::BreakpointCondition::parse(NARROW).unwrap_or_else(|_| {
                adw::BreakpointCondition::new_length(
                    adw::BreakpointConditionLengthType::MaxWidth,
                    640.0,
                    adw::LengthUnit::Sp,
                )
            }));
        narrow.add_setter(
            &header,
            "title-widget",
            Some(&None::<gtk::Widget>.to_value()),
        );
        narrow.add_setter(&bar, "reveal", Some(&true.to_value()));
        self.window.add_breakpoint(narrow);
    }

    /// The read-only, error and connection banners, kept current.
    fn banners(&self) -> [adw::Banner; 3] {
        let read_only = adw::Banner::builder()
            .title(READ_ONLY)
            .use_markup(false)
            .build();
        let error = adw::Banner::builder()
            .use_markup(false)
            .button_label("Dismiss")
            .build();
        let connection = adw::Banner::builder().use_markup(false).build();
        error.connect_button_clicked(glib::clone!(
            #[weak(rename_to = store)]
            self.store,
            move |_| store.clear_error()
        ));
        let show = glib::clone!(
            #[weak]
            read_only,
            #[weak]
            error,
            #[weak]
            connection,
            move |store: &SettingsStore| {
                read_only.set_revealed(store.loaded() && !store.writable());
                let failed = store.error();
                error.set_title(&failed.sentence());
                error.set_revealed(!failed.is_empty());
                let unreachable = store.connection_error();
                connection.set_title(&unreachable.sentence());
                connection.set_revealed(!unreachable.is_empty());
            }
        );
        show(&self.store);
        self.store.connect_changed(show.clone());
        self.store.connect_messages_changed(show);
        [read_only, error, connection]
    }

    fn connect_signals(self: &Rc<Self>) {
        // SET-01, SET-08: the title follows the page, and a page the user
        // chooses is the one the window reopens on.
        self.stack.connect_visible_child_name_notify(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |stack| {
                let Some(id) = stack.visible_child_name() else {
                    return;
                };
                if let Some(info) = pages::info(&id) {
                    this.window.set_title(Some(info.title));
                }
                if !this.selecting.get() {
                    this.navigated.set(true);
                    this.store.remember_page(&id);
                }
            }
        ));
        // SET-08: the last page, once known and unless another was chosen.
        self.store.connect_loaded_notify(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |store| {
                if store.loaded() && !this.navigated.get() {
                    let last = last_page(store);
                    if let Some(page) = last {
                        this.select_page(&page);
                    }
                }
            }
        ));
        // Changes made elsewhere show without a restart: while the window is
        // on screen the store follows the service; when it comes back, all of
        // it is read again.
        self.window.connect_visible_notify(glib::clone!(
            #[weak(rename_to = store)]
            self.store,
            move |window| store.set_live(window.is_visible())
        ));
        // Anything the service changed without announcing it shows once the
        // user returns to the window.
        self.window.connect_is_active_notify(glib::clone!(
            #[weak(rename_to = store)]
            self.store,
            move |window| {
                if window.is_active() && !store.offline() {
                    store.poll();
                }
            }
        ));
        self.add_shortcuts();
    }

    /// SET-07, KEY-50: Escape and Ctrl+W close (hide) the window; Ctrl+Q
    /// quits Wye. A sheet or popover open on top takes Escape first; with a
    /// sheet open, Ctrl+W closes the sheet (as its Cancel does) and never
    /// hides the window from under it, which would drop the sheet's work.
    fn add_shortcuts(&self) {
        let close = gtk::CallbackAction::new(|widget, _| {
            if let Some(window) = widget.downcast_ref::<adw::ApplicationWindow>() {
                match window.visible_dialog() {
                    Some(sheet) => {
                        sheet.close();
                    }
                    None => {
                        window.close();
                    }
                }
            }
            glib::Propagation::Stop
        });
        let quit = gtk::CallbackAction::new(glib::clone!(
            #[weak(rename_to = store)]
            self.store,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |widget, _| {
                store.act(Action::Quit);
                if let Some(window) = widget.downcast_ref::<gtk::Window>() {
                    window.close();
                }
                glib::Propagation::Stop
            }
        ));
        let controller = gtk::ShortcutController::new();
        for (trigger, action) in [
            ("Escape", close.clone()),
            ("<Control>w", close),
            ("<Control>q", quit),
        ] {
            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(action),
            ));
        }
        self.window.add_controller(controller);
    }

    /// Show `id` without remembering it: the page the last session ended on
    /// is not the user's new choice yet.
    fn select_page(&self, id: &str) {
        let id = pages::info(id).map_or("general", |info| info.id);
        self.selecting.set(true);
        self.stack.set_visible_child_name(id);
        self.selecting.set(false);
        if let Some(info) = pages::info(id) {
            self.window.set_title(Some(info.title));
        }
    }

    /// The user (or a caller) goes to a page; the window reopens on it.
    fn show_page(&self, id: &str) {
        self.select_page(id);
        self.navigated.set(true);
        let shown = self.stack.visible_child_name().unwrap_or_default();
        self.store.remember_page(&shown);
    }

    /// `ShowWindow(key, argument)` for this window: open or raise it on the
    /// page asked for (SET-04), see `super::request`.
    pub fn handle(&self, key: &str, argument: &str) {
        let request = Request::parse(key, argument);
        if request.case {
            // Self-test: each case starts from the window alone.
            sheets::close_open(&self.window);
        }
        if let Some(fixture) = &request.fixture
            && let Err(error) = self.store.load_fixture(fixture)
        {
            glib::g_critical!(
                "wye-gtk",
                "settings: the fixture is not service data: {error}"
            );
        }
        if let Some(page) = request.page.as_deref() {
            if pages::info(page).is_none() {
                glib::g_warning!("wye-gtk", "settings: there is no page {page:?}");
            }
            self.show_page(page);
        } else {
            // SET-08: the last page, once it is known.
            let last = last_page(&self.store);
            self.navigated.set(last.is_some());
            let current = self.stack.visible_child_name().unwrap_or_default();
            self.select_page(last.as_deref().unwrap_or(&current));
        }
        if let Some((key, argument)) = &request.rules
            && let Some(page) = self.page(super::request::RULES_PAGE)
        {
            page.request(key, argument);
        }
        if let Some(sheet) = &request.sheet {
            let shown = self.stack.visible_child_name().unwrap_or_default();
            let opened = self.page(&shown).is_some_and(|page| page.open_sheet(sheet));
            if !opened {
                glib::g_warning!(
                    "wye-gtk",
                    "settings: the {shown} page has no sheet {sheet:?}"
                );
            }
        }
        self.window.present();
    }

    fn page(&self, id: &str) -> Option<&dyn Page> {
        self.pages
            .iter()
            .find(|(info, _)| info.id == id)
            .map(|(_, page)| page.as_ref())
    }
}

/// The page the user was on last time (SET-08), once the status is known.
fn last_page(store: &SettingsStore) -> Option<String> {
    if !store.loaded() {
        return None;
    }
    store
        .with_snapshot(|snapshot| snapshot.status.ui_state.last_page.clone())
        .filter(|page| pages::info(page).is_some())
}

/// SET-05: the default height, at most 85 % of the height of the monitor
/// the window opens on, once it is known (the window realized), else of the
/// first monitor.
fn default_height(window: Option<&adw::ApplicationWindow>) -> i32 {
    let display = gdk::Display::default();
    let on_window = window
        .and_then(gtk::prelude::NativeExt::surface)
        .zip(display.clone())
        .and_then(|(surface, display)| display.monitor_at_surface(&surface));
    let first = || {
        display
            .and_then(|display| display.monitors().item(0))
            .and_downcast::<gdk::Monitor>()
    };
    let monitor_height = on_window
        .or_else(first)
        .map(|monitor| monitor.geometry().height());
    match monitor_height {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a share of a monitor's height in pixels fits an i32"
        )]
        Some(height) if height > 0 => DEFAULT_SIZE
            .1
            .min((f64::from(height) * MAX_SCREEN_SHARE) as i32),
        _ => DEFAULT_SIZE.1,
    }
}
