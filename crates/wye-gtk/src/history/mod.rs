//! History (17-dialogs.md, DLG-HIS-01 to DLG-HIS-04): the links Wye opened,
//! newest first and grouped by day, with a search, **Clear History…**, a
//! menu per row (Open in Picker, Open in \<target\> Again, Copy Link, Copy
//! Original Link, Create Rule…, Delete Entry) and the empty states. While
//! the window is open it follows `HistoryRevision` and `InventoryRevision`
//! (`service::watch`), so links opened meanwhile appear without polling.
//!
//! One instance (SET-04): showing it again raises it.
//!
//! GTK-free logic from wye-ui (one source for both frontends; crates/wye-ui
//! owns it): [`filter`] (the search), [`view`] (the rows), [`sync`] (reload
//! and the actions over D-Bus), [`fixture`] (the self-test's data). `view`,
//! `sync` and `fixture` are symlinks, so their nested `tests` modules and
//! `fixtures/history.json` resolve inside this crate.
//!
//! On GTK: [`window`] (the frame), [`row`] (one entry), [`days`] (the day
//! headers and times).
//!
//! The self-test's argument is a JSON object: `fixture` (what `GetHistory`
//! and `GetTargets` would return), `scheme`, `query` (the search text),
//! `now` (Unix seconds the days are counted from), `menu` (open that row's
//! menu) and `confirm` (show the Clear History question).
//!
//! KDE counterpart: crates/wye-ui/qml/history/, crates/wye-ui/src/history/.

mod days;
pub mod fixture;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/history/filter.rs"]
pub mod filter;
mod row;
pub mod sync;
pub mod view;
mod window;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use serde::Deserialize;
use serde_json::{Value, json};
use wye_api::Error;
use wye_api::actions::Reopen;

use crate::app::Presenter;
use crate::service::{self, Change, Subscription};
use crate::widgets::toast;
use fixture::Fixture;
use row::{Act, HistoryRow, RowAction};
use sync::{Action, Known, Snapshot, Update};
use view::View;
use window::{Frame, page};

/// The service properties the window follows (DLG-HIS-01).
const WATCHED: &[&str] = &["HistoryRevision", "InventoryRevision"];

/// How long the self-test lets the window map before it opens a menu.
const MENU_DELAY: Duration = Duration::from_millis(250);

/// What a `ShowWindow("history", …)` argument may hold (the self-test's).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    fixture: Option<Value>,
    query: Option<String>,
    now: Option<i64>,
    menu: Option<usize>,
    #[serde(default)]
    confirm: bool,
}

impl Request {
    fn parse(argument: &str) -> Option<Self> {
        if !argument.trim_start().starts_with('{') {
            return None;
        }
        serde_json::from_str(argument)
            .inspect_err(|error| tracing::warn!(%error, "history: the argument is not a request"))
            .ok()
    }
}

/// One day on screen: its group and its rows with their entry IDs.
type DayGroup = (adw::PreferencesGroup, Vec<(u64, HistoryRow)>);

/// The History surface.
#[derive(Debug)]
pub struct History {
    app: adw::Application,
    controller: OnceCell<Rc<Controller>>,
}

impl History {
    /// The surface; the window is built when first shown.
    #[must_use]
    pub fn new(app: &adw::Application) -> Self {
        Self {
            app: app.clone(),
            controller: OnceCell::new(),
        }
    }
}

impl Presenter for History {
    fn present(&self, _key: &str, argument: &str) {
        let controller = self.controller.get_or_init(|| Controller::new(&self.app));
        controller.show(argument);
    }
}

/// The window and what it shows.
struct Controller {
    frame: Frame,
    me: Weak<Self>,
    snapshot: RefCell<Snapshot>,
    /// A fixture stands in for the service (self-test).
    offline: Cell<bool>,
    /// The service answered once (or a fixture was loaded).
    loaded: Cell<bool>,
    /// The moment the days are counted from; now when unset.
    now: Cell<Option<i64>>,
    twelve_hour: bool,
    /// The rows on screen, by day, for the search.
    days: RefCell<Vec<DayGroup>>,
    /// Followed while the window is open.
    subscription: RefCell<Option<Subscription>>,
}

impl std::fmt::Debug for Controller {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Controller")
            .field("offline", &self.offline.get())
            .finish_non_exhaustive()
    }
}

impl Controller {
    fn new(app: &adw::Application) -> Rc<Self> {
        let controller = Rc::new_cyclic(|me| Self {
            frame: Frame::new(app),
            me: me.clone(),
            snapshot: RefCell::default(),
            offline: Cell::new(false),
            loaded: Cell::new(false),
            now: Cell::new(None),
            twelve_hour: days::twelve_hour_clock(),
            days: RefCell::default(),
            subscription: RefCell::default(),
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
        let frame = &self.frame;
        let search = self.with(Self::filter);
        frame.search_entry.connect_search_changed(move |_| search());
        let turn_on = self.with(|this| this.act(Action::TurnOn));
        frame.turn_on.connect_clicked(move |_| turn_on());
        let ask = self.with(Self::ask_clear);
        frame.clear.connect_activate(move |_, _| ask());
        // DLG-HIS-01: follow the service only while the window is open.
        let stop = self.with(|this| drop(this.subscription.take()));
        frame.window.connect_close_request(move |_| {
            stop();
            glib::Propagation::Proceed
        });
    }

    /// Show, raise and focus the window (SET-04) for `argument`.
    fn show(&self, argument: &str) {
        let request = Request::parse(argument);
        if request.is_some() {
            // The self-test's next case: whatever the last one opened goes.
            if let Some(dialog) = self.frame.window.visible_dialog() {
                dialog.force_close();
            }
        }
        let request = request.unwrap_or_default();
        if let Some(fixture) = &request.fixture {
            self.load_fixture(fixture);
        }
        if let Some(now) = request.now {
            self.now.set(Some(now));
        }
        self.render();
        if let Some(query) = &request.query {
            self.frame.search_entry.set_text(query);
            self.frame.search_bar.set_search_mode(!query.is_empty());
            self.filter();
        }
        if !self.offline.get() {
            self.follow();
            self.reload(Known::default());
        }
        self.frame.window.present();
        if request.confirm {
            self.ask_clear();
        }
        if let Some(index) = request.menu {
            self.open_menu(index);
        }
    }

    fn load_fixture(&self, fixture: &Value) {
        match Fixture::parse(&fixture.to_string()) {
            Ok(fixture) => {
                self.offline.set(true);
                self.loaded.set(true);
                self.subscription.take();
                self.snapshot.replace(fixture.snapshot());
            }
            Err(error) => {
                glib::g_critical!(
                    "wye-gtk",
                    "history: the fixture is not service data: {error}"
                );
            }
        }
    }

    /// Follow the service's revisions while the window is open.
    fn follow(&self) {
        if self.subscription.borrow().is_some() {
            return;
        }
        let me = self.me.clone();
        let subscription = service::watch(WATCHED, move |change| {
            let Some(controller) = me.upgrade() else {
                return;
            };
            let known = match change {
                Change::Moved => controller.snapshot.borrow().known,
                // A new service starts its revisions over: read everything.
                Change::Restarted => Known::default(),
            };
            controller.reload(known);
        });
        self.subscription.replace(Some(subscription));
    }

    /// Read what changed since `known`.
    fn reload(&self, known: Known) {
        let me = self.me.clone();
        service::request(
            move |proxy| async move { sync::poll(&proxy, known).await },
            move |result: Result<Option<Update>, Error>| {
                if let Some(controller) = me.upgrade() {
                    controller.received(result);
                }
            },
        );
    }

    /// Carry out `action` on the service, then show what it changed.
    fn act(&self, action: Action) {
        if self.offline.get() {
            tracing::info!(?action, "a history action in the self-test");
            return;
        }
        let known = self.snapshot.borrow().known;
        let me = self.me.clone();
        service::request(
            move |proxy| async move { sync::run(&proxy, action, known).await },
            move |result: Result<Option<Update>, Error>| {
                if let Some(controller) = me.upgrade() {
                    controller.received(result);
                }
            },
        );
    }

    fn received(&self, result: Result<Option<Update>, Error>) {
        if self.offline.get() {
            // An answer that was on its way when a fixture took over.
            return;
        }
        match result {
            Ok(update) => {
                self.frame.set_error("");
                let first = !self.loaded.replace(true);
                if let Some(update) = update {
                    let next = update.apply(&self.snapshot.borrow());
                    self.snapshot.replace(next);
                    self.render();
                } else if first {
                    self.render();
                }
            }
            Err(error) => {
                tracing::warn!(%error, "a history request failed");
                self.frame
                    .set_error(&crate::error_text::describe(&error).sentence());
            }
        }
    }

    /// Rebuild the list from the snapshot (DLG-HIS-02, DLG-HIS-04).
    fn render(&self) {
        for (group, _) in self.days.take() {
            self.frame.list.remove(&group);
        }
        let snapshot = self.snapshot.borrow();
        let view = View::build(&snapshot.history, &snapshot.targets, "");
        self.frame.set_has_history(view.total > 0);
        let now = self
            .now
            .get()
            .unwrap_or_else(|| glib::DateTime::now_utc().map_or(0, |now| now.to_unix()));
        let zone = glib::TimeZone::local();
        let times: Vec<i64> = view.rows.iter().map(|row| row.time).collect();
        let act = self.row_act();
        let mut built = Vec::new();
        for day in days::group(&times, now, &zone) {
            let group = adw::PreferencesGroup::builder().title(&day.title).build();
            let list = gtk::ListBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .activate_on_single_click(false)
                .build();
            list.add_css_class("boxed-list");
            let mut rows = Vec::new();
            // `days::group` hands out indices of `times`, one per row.
            for data in day
                .rows
                .into_iter()
                .filter_map(|index| view.rows.get(index))
            {
                let time = days::time_of_day(data.time, &zone, self.twelve_hour);
                let badge = badge_of(&snapshot, data.id);
                let shown = row::build(data, &time, badge.as_ref(), &act);
                list.append(&shown.row);
                rows.push((data.id, shown));
            }
            // DLG-HIS-03: double-click or Enter opens the link in the picker.
            let ids: Vec<u64> = rows.iter().map(|(id, _)| *id).collect();
            let open = Rc::clone(&act);
            list.connect_row_activated(move |_, row| {
                if let Some(&id) = usize::try_from(row.index()).ok().and_then(|i| ids.get(i)) {
                    open(RowAction::Picker(id));
                }
            });
            group.add(&list);
            self.frame.list.add(&group);
            built.push((group, rows));
        }
        self.days.replace(built);
        drop(snapshot);
        self.filter();
    }

    /// Show only the rows the search keeps (DLG-HIS-01), and pick the page.
    fn filter(&self) {
        let snapshot = self.snapshot.borrow();
        let history = &snapshot.history;
        let query = self.frame.search_entry.text();
        let matching: HashSet<u64> = filter::matching(&history.entries, &query)
            .iter()
            .map(|entry| entry.id)
            .collect();
        let mut any = false;
        for (group, rows) in self.days.borrow().iter() {
            let mut shown = false;
            for (id, row) in rows {
                let keep = matching.contains(id);
                row.row.set_visible(keep);
                shown |= keep;
            }
            group.set_visible(shown);
            any |= shown;
        }
        let name = if !self.loaded.get() {
            page::LOADING
        } else if !history.enabled {
            page::OFF
        } else if history.entries.is_empty() {
            page::EMPTY
        } else if any {
            page::LIST
        } else {
            let text = glib::markup_escape_text(query.as_str());
            self.frame
                .no_matches
                .set_description(Some(&format!("No link matches “{text}”.")));
            page::NO_MATCHES
        };
        self.frame.show_page(name);
    }

    /// What a row's menu item does.
    fn row_act(&self) -> Act {
        let me = self.me.clone();
        Rc::new(move |action| {
            if let Some(controller) = me.upgrade() {
                controller.on_row(action);
            }
        })
    }

    fn on_row(&self, action: RowAction) {
        match action {
            RowAction::Picker(id) => self.act(Action::Reopen(id, Reopen::Picker)),
            RowAction::SameTarget(id) => self.act(Action::Reopen(id, Reopen::SameTarget)),
            RowAction::Delete(id) => self.act(Action::Delete(id)),
            RowAction::Copy(text) => {
                self.frame.window.clipboard().set_text(&text);
                toast::show(&self.frame.window, adw::Toast::new("Link copied"));
            }
            RowAction::CreateRule(id) => self.create_rule(id),
        }
    }

    /// DLG-HIS-03, PICK-31: the rule editor, pre-filled with the entry's
    /// host and source app.
    fn create_rule(&self, id: u64) {
        let prefill = {
            let snapshot = self.snapshot.borrow();
            let Some(entry) = snapshot.history.entries.iter().find(|e| e.id == id) else {
                return;
            };
            let (host, _) = view::split_url(&entry.final_url);
            json!({"domain": host, "sourceApp": entry.source}).to_string()
        };
        self.act(Action::CreateRule { prefill });
    }

    /// DLG-HIS-01: ask before forgetting everything.
    fn ask_clear(&self) {
        let dialog = window::confirm_clear();
        let clear = self.with(|this| this.act(Action::Clear));
        dialog.connect_response(Some("clear"), move |_, _| clear());
        dialog.present(Some(&self.frame.window));
    }

    /// The self-test: open the menu of the row at `index`, once it is shown.
    fn open_menu(&self, index: usize) {
        let button = self
            .days
            .borrow()
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .nth(index)
            .map(|(_, row)| row.menu_button.clone());
        if let Some(button) = button {
            glib::timeout_add_local_once(MENU_DELAY, move || button.popup());
        }
    }
}

/// The profile badge of the target entry `id` went to, as the icon takes it.
fn badge_of(snapshot: &Snapshot, id: u64) -> Option<Value> {
    let entry = snapshot.history.entries.iter().find(|e| e.id == id)?;
    let info = snapshot
        .targets
        .targets
        .iter()
        .find(|info| info.target == entry.target)?;
    serde_json::to_value(info.badge.as_ref()?).ok()
}
