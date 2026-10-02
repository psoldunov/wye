//! The app chooser, "Choose App" (DLG-APP-01 to DLG-APP-04): a modal sheet
//! with a search entry under its header bar (focused on open, and typing
//! anywhere in the sheet goes there) that filters by name, generic name,
//! desktop ID and keywords; the apps in sections **Recent Sources** (only
//! when choosing source apps), **Browsers** and **All Apps**, each row with
//! the app's icon, its name and a dimmed packaging badge ("Flatpak",
//! "Snap"). **Browse…** at the bottom picks an executable or a `.desktop`
//! file for an app without an installed desktop entry.
//!
//! Single choice (a target: "Other…" of every target menu, TGT-06, and
//! "Add App…" of the shown browsers sheet, SHOWN-05): a click chooses and
//! closes. Multiple choice (source apps): check boxes, and **Add** confirms.
//! Enter in the search entry chooses the first app.
//!
//! The rows and the targets come from the shared model
//! (`crate::settings::chooser`); the apps from `GetApps(true)` through the
//! store (`SettingsStore::apps`), or the fixture's list in the self-test.
//!
//! KDE counterpart: crates/wye-ui/qml/components/WyeAppChooser.qml.
//!
//! API:
//! - [`AppChooser::open`]`(parent, store, choice, chosen)`: build and show
//!   it over `parent`'s window; `chosen(targets)` gets the targets in their
//!   configuration shape (`{"custom": "slack.desktop"}`), once.
//! - [`AppChooser::set_search`]: type into the search entry (self-test).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;
use wye_api::apps::AppList;

use crate::error_text;
use crate::settings::chooser::{self, RowKind};
use crate::settings::store::SettingsStore;
use crate::widgets::{empty_state, icon};

/// The sheet's size in logical pixels.
const SIZE: (i32, i32) = (440, 620);
/// App icons in the list.
const ICON_SIZE: i32 = 32;

/// How many apps the sheet chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// A target: a click chooses and closes.
    Single,
    /// Source apps: check boxes and **Add**; `recent` lists the apps that
    /// sent links this session first (DLG-APP-02).
    Multiple { recent: bool },
}

impl Choice {
    fn recent(self) -> bool {
        matches!(self, Self::Multiple { recent: true })
    }
}

type Chosen = dyn Fn(&[Value]);

/// What the sheet's stack shows.
mod view {
    pub const LOADING: &str = "loading";
    pub const LIST: &str = "list";
    pub const EMPTY: &str = "empty";
    pub const ERROR: &str = "error";
}

/// A section of the list (Recent Sources, Browsers, All Apps) and its rows,
/// by desktop ID.
struct Section {
    group: adw::PreferencesGroup,
    rows: Vec<(String, adw::ActionRow)>,
}

struct Inner {
    dialog: adw::Dialog,
    search: gtk::SearchEntry,
    stack: gtk::Stack,
    page: adw::PreferencesPage,
    error: adw::StatusPage,
    add: gtk::Button,
    /// Every section with its rows, built once per app list; a search only
    /// shows and hides them.
    sections: RefCell<Vec<Section>>,
    apps: RefCell<AppList>,
    /// Desktop IDs checked so far (multiple choice).
    selected: RefCell<Vec<String>>,
    choice: Choice,
    chosen: Box<Chosen>,
    /// Told which app a single choice picked, so its row has a name at once
    /// (TGT-06).
    store: SettingsStore,
}

/// An open app chooser. Clones share it.
#[derive(Clone)]
pub struct AppChooser {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for AppChooser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppChooser")
            .field("choice", &self.inner.choice)
            .finish_non_exhaustive()
    }
}

impl AppChooser {
    /// Build the sheet, show it over `parent`'s window and list the apps.
    pub fn open(
        parent: &impl IsA<gtk::Widget>,
        store: &SettingsStore,
        choice: Choice,
        chosen: impl Fn(&[Value]) + 'static,
    ) -> Self {
        let this = Self::build(store, choice, Box::new(chosen));
        this.inner.dialog.present(Some(parent));
        this.inner.dialog.set_focus(Some(&this.inner.search));
        let weak = Rc::downgrade(&this.inner);
        store.apps(move |result| {
            if let Some(inner) = weak.upgrade() {
                loaded(&inner, result);
            }
        });
        this
    }

    /// Type `text` into the search entry.
    pub fn set_search(&self, text: &str) {
        self.inner.search.set_text(text);
        refresh(&self.inner);
    }

    fn build(store: &SettingsStore, choice: Choice, chosen: Box<Chosen>) -> Self {
        let multiple = matches!(choice, Choice::Multiple { .. });
        let cancel = gtk::Button::with_label("Cancel");
        let add = gtk::Button::builder()
            .css_classes(["suggested-action"])
            .label("Add")
            .sensitive(false)
            .visible(multiple)
            .build();
        let header = adw::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();
        header.pack_start(&cancel);
        header.pack_end(&add);
        let (search, search_bar) = search_bar();
        let page = adw::PreferencesPage::new();
        let error = empty_state::empty_state("dialog-warning-symbolic", "Cannot List Apps", "");
        let stack = views(&page, &error);
        let (browse, bottom) = browse_bar();

        let toolbar = adw::ToolbarView::builder().content(&stack).build();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&search_bar);
        toolbar.add_bottom_bar(&bottom);
        let dialog = adw::Dialog::builder()
            .title("Choose App")
            .child(&toolbar)
            .content_width(SIZE.0)
            .content_height(SIZE.1)
            .build();
        search.set_key_capture_widget(Some(&dialog));

        let inner = Rc::new(Inner {
            dialog,
            search,
            stack,
            page,
            error,
            add,
            sections: RefCell::default(),
            apps: RefCell::default(),
            selected: RefCell::default(),
            choice,
            chosen,
            store: store.clone(),
        });
        connect(&inner, &cancel, &browse);
        // The sheet keeps itself until it closes; its widgets hold it weakly.
        let keep = RefCell::new(Some(Rc::clone(&inner)));
        inner.dialog.connect_closed(move |_| {
            keep.take();
        });
        Self { inner }
    }
}

/// The search entry under the header bar, named for screen readers
/// (DLG-APP-01).
fn search_bar() -> (gtk::SearchEntry, gtk::Box) {
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search apps")
        .hexpand(true)
        .build();
    search.update_property(&[gtk::accessible::Property::Label("Search apps")]);
    let bar = gtk::Box::builder()
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(6)
        .build();
    bar.append(&search);
    (search, bar)
}

/// What the sheet shows: loading, the list on `page`, no app found, or
/// `error`.
fn views(page: &adw::PreferencesPage, error: &adw::StatusPage) -> gtk::Stack {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    stack.add_named(&loading(), Some(view::LOADING));
    stack.add_named(page, Some(view::LIST));
    stack.add_named(
        &empty_state::empty_state(
            "edit-find-symbolic",
            "No Apps Found",
            "Try another search, or find the app with Browse…",
        ),
        Some(view::EMPTY),
    );
    stack.add_named(error, Some(view::ERROR));
    stack.set_visible_child_name(view::LOADING);
    stack
}

/// DLG-APP-04: **Browse…** in a bar at the bottom.
fn browse_bar() -> (gtk::Button, gtk::Box) {
    let browse = gtk::Button::builder()
        .child(
            &adw::ButtonContent::builder()
                .icon_name("document-open-symbolic")
                .label("Browse…")
                .build(),
        )
        .tooltip_text("Choose a program or a .desktop file")
        .build();
    browse.update_property(&[gtk::accessible::Property::Label("Browse…")]);
    let bottom = gtk::Box::builder().build();
    bottom.add_css_class("toolbar");
    bottom.append(&browse);
    (browse, bottom)
}

/// Wire the buttons and the search entry.
fn connect(inner: &Rc<Inner>, cancel: &gtk::Button, browse: &gtk::Button) {
    let weak = Rc::downgrade(inner);
    cancel.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| with(&weak, |inner| {
            inner.dialog.close();
        })
    ));
    inner.add.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| with(&weak, confirm)
    ));
    browse.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| with(&weak, browse_file)
    ));
    inner.search.connect_search_changed(glib::clone!(
        #[strong]
        weak,
        move |_| with(&weak, refresh)
    ));
    // DLG-APP-03: Enter takes the first app of a single choice.
    inner.search.connect_activate(move |_| {
        with(&weak, |inner| {
            if inner.choice != Choice::Single {
                return;
            }
            let first = chooser::rows(&inner.apps.borrow(), &inner.search.text(), false)
                .into_iter()
                .find(|row| row.kind == RowKind::App);
            if let Some(row) = first {
                choose(inner, &row.id);
            }
        });
    });
}

fn with(weak: &Weak<Inner>, act: impl FnOnce(&Rc<Inner>)) {
    if let Some(inner) = weak.upgrade() {
        act(&inner);
    }
}

/// The apps arrived (or could not be read).
fn loaded(inner: &Rc<Inner>, result: Result<AppList, wye_api::Error>) {
    match result {
        Ok(apps) => {
            inner.apps.replace(apps);
            build_sections(inner);
            refresh(inner);
        }
        Err(error) => {
            tracing::warn!(%error, "cannot list the installed apps");
            inner
                .error
                .set_description(Some(&error_text::describe(&error).sentence()));
            inner.stack.set_visible_child_name(view::ERROR);
        }
    }
}

/// Build every row of the app list, in its sections (DLG-APP-02): what
/// no search hides.
fn build_sections(inner: &Rc<Inner>) {
    for section in inner.sections.take() {
        inner.page.remove(&section.group);
    }
    let rows = chooser::rows(&inner.apps.borrow(), "", inner.choice.recent());
    let mut sections: Vec<Section> = Vec::new();
    for row in &rows {
        if row.kind == RowKind::Header || sections.is_empty() {
            let group = adw::PreferencesGroup::new();
            if row.kind == RowKind::Header {
                group.set_title(&row.label);
            }
            inner.page.add(&group);
            sections.push(Section {
                group,
                rows: Vec::new(),
            });
        }
        if row.kind == RowKind::App
            && let Some(section) = sections.last_mut()
        {
            let widget = app_row(inner, row);
            section.group.add(&widget);
            section.rows.push((row.id.clone(), widget));
        }
    }
    inner.sections.replace(sections);
}

/// Show the rows the current search finds (DLG-APP-01): the rows are
/// built once, so typing never builds the list again.
fn refresh(inner: &Rc<Inner>) {
    if inner.stack.visible_child_name().as_deref() == Some(view::ERROR) {
        return;
    }
    let found = chooser::rows(
        &inner.apps.borrow(),
        &inner.search.text(),
        inner.choice.recent(),
    );
    let ids: HashSet<&str> = found
        .iter()
        .filter(|row| row.kind == RowKind::App)
        .map(|row| row.id.as_str())
        .collect();
    for section in inner.sections.borrow().iter() {
        let mut any = false;
        for (id, row) in &section.rows {
            let shown = ids.contains(id.as_str());
            row.set_visible(shown);
            any |= shown;
        }
        section.group.set_visible(any);
    }
    inner.stack.set_visible_child_name(if ids.is_empty() {
        view::EMPTY
    } else {
        view::LIST
    });
}

/// One app: its icon, name and packaging badge; a check box when several
/// are chosen.
fn app_row(inner: &Rc<Inner>, row: &chooser::Row) -> adw::ActionRow {
    let action = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&row.label))
        .activatable(true)
        .build();
    // `add_prefix` puts each prefix before the ones already there: the icon
    // first, then a check box ahead of it.
    let source = if row.icon.is_empty() {
        icon::FALLBACK_APP_ICON
    } else {
        row.icon.as_str()
    };
    action.add_prefix(&icon::image(source, ICON_SIZE));
    let weak = Rc::downgrade(inner);
    let id = row.id.clone();
    if inner.choice == Choice::Single {
        action.connect_activated(move |_| with(&weak, |inner| choose(inner, &id)));
    } else {
        let check = gtk::CheckButton::builder()
            .active(inner.selected.borrow().contains(&row.id))
            .valign(gtk::Align::Center)
            .build();
        check.update_property(&[gtk::accessible::Property::Label(&row.label)]);
        action.add_prefix(&check);
        action.set_activatable_widget(Some(&check));
        check.connect_toggled(move |check| {
            with(&weak, |inner| toggle(inner, &id, check.is_active()));
        });
    }
    if !row.packaging.is_empty() {
        let badge = gtk::Label::builder()
            .label(&row.packaging)
            .valign(gtk::Align::Center)
            .build();
        badge.add_css_class("wye-packaging-badge");
        action.add_suffix(&badge);
    }
    action
}

/// DLG-APP-03, single choice: the app with desktop ID `id` is the answer.
fn choose(inner: &Rc<Inner>, id: &str) {
    let picked = inner
        .apps
        .borrow()
        .apps
        .iter()
        .find(|app| app.id == id)
        .map(|app| (chooser::target_for(app), chooser::chosen_info(app)));
    if let Some((target, info)) = picked {
        // Before the answer: the row saves on it and must already resolve.
        inner.store.remember_chosen(info);
        answer(inner, &[target]);
    }
}

/// DLG-APP-03, multiple choice: a check box flipped.
fn toggle(inner: &Rc<Inner>, id: &str, checked: bool) {
    {
        let mut selected = inner.selected.borrow_mut();
        selected.retain(|existing| existing != id);
        if checked {
            selected.push(id.to_owned());
        }
    }
    inner.add.set_sensitive(!inner.selected.borrow().is_empty());
}

/// **Add**: the checked apps, in the order they were checked.
fn confirm(inner: &Rc<Inner>) {
    let targets: Vec<Value> = {
        let apps = inner.apps.borrow();
        inner
            .selected
            .borrow()
            .iter()
            .filter_map(|id| apps.apps.iter().find(|app| app.id == *id))
            .map(chooser::target_for)
            .collect()
    };
    answer(inner, &targets);
}

/// DLG-APP-04: pick a program or a `.desktop` file.
fn browse_file(inner: &Rc<Inner>) {
    let dialog = gtk::FileDialog::builder()
        .title("Choose an App")
        .modal(true)
        .build();
    let window = inner.dialog.root().and_downcast::<gtk::Window>();
    let weak = Rc::downgrade(inner);
    dialog.open(window.as_ref(), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else {
            // Dismissed: nothing to do.
            return;
        };
        let Some(path) = file.path() else {
            return;
        };
        let path = path.to_string_lossy();
        if let Some(target) = chooser::browse_target(&path) {
            with(&weak, |inner| {
                if inner.choice == Choice::Single
                    && let Some(info) = chooser::browse_info(&path, &inner.apps.borrow())
                {
                    inner.store.remember_chosen(info);
                }
                answer(inner, &[target]);
            });
        }
    });
}

/// Hand `targets` over and close.
fn answer(inner: &Rc<Inner>, targets: &[Value]) {
    if !targets.is_empty() {
        (inner.chosen)(targets);
    }
    inner.dialog.close();
}

/// The list while `GetApps` is on its way.
fn loading() -> gtk::Widget {
    let spinner = adw::Spinner::builder()
        .width_request(32)
        .height_request(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    spinner.upcast()
}
