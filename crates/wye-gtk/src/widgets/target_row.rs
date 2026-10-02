//! BLK-04 Target popup row: an `AdwComboRow` whose value is the current
//! target's icon and name, opening the target menu (TGT-01 to TGT-07): the
//! sections of `crate::settings::menu` with their headers ("Private
//! Browsing", "Profiles: Chrome") and a line between the others, a check on
//! the current value (TGT-03), a warning on a target whose app is gone
//! (APP-10), and "Other…" last, which opens the app chooser (TGT-06) and
//! leaves the value as it is.
//!
//! The menu comes from the store ([`SettingsStore::target_menu`]), so it
//! follows `GetTargets` without the page doing anything.
//!
//! API:
//! - [`TargetRow::new`]`(title, subtitle, surface)`: the row; `surface`
//!   decides the sections (`Browsers`, `Apps`, `Rule`).
//! - [`TargetRow::bind`]`(store, path, default)`: show the target at `path`
//!   (`browsers.primary`), save a choice with `set_target` (SET-06), follow
//!   `writable`.
//! - [`TargetRow::bind_service`]`(store, service)`: the same for a web
//!   service's mapping on the Apps page (APP-04, `set_service_target`).
//! - [`TargetRow::connect_other`]: what "Other…" does (open the app
//!   chooser, DLG-APP); without it "Other…" does nothing.
//! - [`TargetRow::show`]`(store, current)` and [`TargetRow::connect_chosen`]
//!   for a row that is not tied to the configuration (the rule editor's
//!   draft, RUL-12).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{Value, json};

use super::{icon, row};
use crate::settings::menu::{Row, RowKind, Surface};
use crate::settings::store::SettingsStore;

/// Icon size in the row and the menu.
const ICON_SIZE: i32 = 16;

/// One menu entry, as the list model holds it.
#[derive(Debug, Clone)]
struct Entry {
    row: Row,
    /// The section's header, on the section's first entry only.
    header: Option<String>,
    /// What the closed row shows while this entry is the value, on the
    /// current entry only: the target's full name, "Work (Google Chrome)"
    /// where the menu's profile section says "Work" (TGT-01).
    shown: Option<Row>,
}

type Chosen = dyn Fn(&Value);
type Other = dyn Fn();

#[derive(Default)]
struct State {
    /// The menu rows and the closed row's value shown, to skip rebuilding
    /// an unchanged menu.
    rows: RefCell<Option<(Vec<Row>, Row)>>,
    /// The entries in model order.
    entries: RefCell<Vec<Entry>>,
    /// The position of the current value.
    current: Cell<u32>,
    /// Set while code changes the model or selection.
    updating: Cell<bool>,
    chosen: RefCell<Option<Rc<Chosen>>>,
    other: RefCell<Option<Rc<Other>>>,
    /// What the menu was last built from, to describe a pick with.
    source: RefCell<Option<Source>>,
}

/// The store, surface and service a menu was built from.
#[derive(Clone)]
struct Source {
    store: glib::WeakRef<SettingsStore>,
    surface: Surface,
    service: String,
}

/// A target popup row. Clones share the row.
#[derive(Clone)]
pub struct TargetRow {
    row: adw::ComboRow,
    surface: Surface,
    state: Rc<State>,
}

impl std::fmt::Debug for TargetRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TargetRow")
            .field("row", &self.row)
            .field("surface", &self.surface)
            .finish_non_exhaustive()
    }
}

impl TargetRow {
    /// A row titled `title` for the menu of `surface`.
    #[must_use]
    pub fn new(title: &str, subtitle: &str, surface: Surface) -> Self {
        let row = row::titled(adw::ComboRow::new(), title, subtitle);
        // The menu's section styling (`data/style.css`) is this row's alone.
        row.add_css_class("wye-target-row");
        row.set_factory(Some(&item_factory(false)));
        row.set_list_factory(Some(&item_factory(true)));
        row.set_header_factory(Some(&header_factory()));
        let state: Rc<State> = Rc::default();
        let this = Self {
            row,
            surface,
            state,
        };
        let state = Rc::clone(&this.state);
        this.row
            .connect_selected_notify(move |row| picked(row, &state));
        this
    }

    /// The row, to add to a group.
    #[must_use]
    pub fn row(&self) -> &adw::ComboRow {
        &self.row
    }

    /// Run `chosen(target)` when the user picks another target.
    pub fn connect_chosen(&self, chosen: impl Fn(&Value) + 'static) {
        self.state.chosen.replace(Some(Rc::new(chosen)));
    }

    /// Run `other()` when the user picks "Other…" (TGT-06).
    pub fn connect_other(&self, other: impl Fn() + 'static) {
        self.state.other.replace(Some(Rc::new(other)));
    }

    /// Show the menu for `current` (the configuration's shape), with
    /// `service` the mapped web service on the Apps page (else empty).
    pub fn show(&self, store: &SettingsStore, current: &Value, service: &str) {
        refresh(
            &self.row,
            self.surface,
            &self.state,
            store,
            current,
            service,
        );
    }

    /// Tie the row to the target at `path`, `default` when the file does not
    /// set it. See the module docs.
    pub fn bind(&self, store: &SettingsStore, path: &'static str, default: &Value) {
        let default = default.clone();
        let current =
            move |store: &SettingsStore| store.value(path).unwrap_or_else(|| default.clone());
        self.follow(store, current.clone(), "");
        self.connect_chosen(glib::clone!(
            #[weak]
            store,
            move |target| {
                if *target != current(&store) {
                    store.set_target(path, target);
                }
            }
        ));
    }

    /// Tie the row to the mapping of web service `service` (APP-04, APP-06).
    pub fn bind_service(&self, store: &SettingsStore, service: &str) {
        let id: Rc<str> = Rc::from(service);
        let current = {
            let id = Rc::clone(&id);
            move |store: &SettingsStore| {
                store.with_snapshot(|snapshot| {
                    snapshot
                        .services
                        .services
                        .iter()
                        .find(|candidate| *candidate.id == *id)
                        .map_or_else(|| json!({"default": true}), |info| info.target.clone())
                })
            }
        };
        self.follow(store, current.clone(), service);
        self.connect_chosen(glib::clone!(
            #[weak]
            store,
            move |target| {
                if *target != current(&store) {
                    store.set_service_target(&id, target);
                }
            }
        ));
    }

    /// Show `current(store)` now and after every store change, and follow
    /// `writable`.
    fn follow(
        &self,
        store: &SettingsStore,
        current: impl Fn(&SettingsStore) -> Value + 'static,
        service: &str,
    ) {
        let service = service.to_owned();
        let surface = self.surface;
        let state = Rc::clone(&self.state);
        let show = glib::clone!(
            #[weak(rename_to = row)]
            self.row,
            move |store: &SettingsStore| {
                refresh(&row, surface, &state, store, &current(store), &service);
            }
        );
        show(store);
        store.connect_changed_while(&self.row, show);
        row::follow_writable(store, &self.row, |_| true);
    }
}

/// Rebuild the menu of `row` for `current`, unless it is unchanged.
fn refresh(
    row: &adw::ComboRow,
    surface: Surface,
    state: &State,
    store: &SettingsStore,
    current: &Value,
    service: &str,
) {
    state.source.replace(Some(Source {
        store: store.downgrade(),
        surface,
        service: service.to_owned(),
    }));
    let mut rows = store.target_menu(surface, current, service);
    let described = Row {
        checked: true,
        ..store.target_label(surface, current, service)
    };
    if !rows.iter().any(|row| row.checked) {
        // TGT-01, APP-10: a value the menu does not offer still shows.
        let separator = Row {
            kind: RowKind::Separator,
            label: String::new(),
            icon: String::new(),
            badge: None,
            target: Value::Null,
            checked: false,
            missing: false,
        };
        rows.splice(0..0, [described.clone(), separator]);
    }
    let shown = (rows, described);
    if state.rows.borrow().as_ref() == Some(&shown) {
        return;
    }
    let entries: Vec<Entry> = entries(&shown.0)
        .into_iter()
        .map(|entry| Entry {
            shown: entry.row.checked.then(|| shown.1.clone()),
            ..entry
        })
        .collect();
    let selected = entries
        .iter()
        .position(|entry| entry.row.checked)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(gtk::INVALID_LIST_POSITION);
    state.updating.set(true);
    row.set_model(Some(&model(&entries)));
    row.set_selected(selected);
    state.updating.set(false);
    state.current.set(selected);
    state.rows.replace(Some(shown));
    state.entries.replace(entries);
}

/// The selection moved: a user's pick, unless code is updating the row.
fn picked(row: &adw::ComboRow, state: &Rc<State>) {
    if state.updating.get() {
        return;
    }
    let selected = row.selected();
    let entry = usize::try_from(selected)
        .ok()
        .and_then(|index| state.entries.borrow().get(index).cloned());
    let Some(entry) = entry else {
        return;
    };
    match entry.row.kind {
        RowKind::Other => {
            // TGT-06: "Other…" opens the chooser; the value stays.
            state.updating.set(true);
            row.set_selected(state.current.get());
            state.updating.set(false);
            let other = state.other.borrow().clone();
            if let Some(other) = other {
                other();
            }
        }
        RowKind::Item => {
            state.current.set(selected);
            commit_pick(row, state, entry.row.target);
        }
        RowKind::Header | RowKind::Separator => {}
    }
}

/// Hand a pick on (SET-06) and name it in full (TGT-01), once the drop-down
/// has finished with the selection. The selection changes inside the menu
/// list's activation, and a bound row's save emits the store's `changed`
/// at once, which rebuilds the menu: a new model while GTK is still
/// activating the item frees the selection it is notifying, a crash. After
/// the store change the rebuild for the pick finds the menu unchanged.
fn commit_pick(row: &adw::ComboRow, state: &Rc<State>, target: Value) {
    let (row, state) = (row.downgrade(), Rc::clone(state));
    glib::idle_add_local_once(move || {
        let Some(row) = row.upgrade() else {
            return;
        };
        let chosen = state.chosen.borrow().clone();
        if let Some(chosen) = chosen {
            chosen(&target);
        }
        let Some(source) = state.source.borrow().clone() else {
            return;
        };
        if let Some(store) = source.store.upgrade() {
            refresh(
                &row,
                source.surface,
                &state,
                &store,
                &target,
                &source.service,
            );
        }
    });
}

/// The menu rows as entries: separators and headers become section breaks
/// and section titles.
fn entries(rows: &[Row]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut header: Option<String> = None;
    let mut first_of_section = true;
    for row in rows {
        match row.kind {
            RowKind::Separator => {
                header = None;
                first_of_section = true;
            }
            RowKind::Header => header = Some(row.label.clone()),
            RowKind::Item | RowKind::Other => {
                let section_header = if first_of_section {
                    Some(header.take().unwrap_or_default())
                } else {
                    None
                };
                first_of_section = false;
                entries.push(Entry {
                    row: row.clone(),
                    header: section_header,
                    shown: None,
                });
            }
        }
    }
    entries
}

/// A sectioned model: one inner list per section, flattened (a
/// `GtkFlattenListModel` reports each inner list as a section).
fn model(entries: &[Entry]) -> gtk::FlattenListModel {
    let sections = gio::ListStore::new::<gio::ListStore>();
    let mut section: Option<gio::ListStore> = None;
    for entry in entries {
        if entry.header.is_some() || section.is_none() {
            if let Some(done) = section.take() {
                sections.append(&done);
            }
            section = Some(gio::ListStore::new::<glib::BoxedAnyObject>());
        }
        if let Some(section) = &section {
            section.append(&glib::BoxedAnyObject::new(entry.clone()));
        }
    }
    if let Some(done) = section {
        sections.append(&done);
    }
    gtk::FlattenListModel::new(Some(sections))
}

/// Icon and name; in the menu also the check (TGT-03) and the missing mark
/// (APP-10).
fn item_factory(in_menu: bool) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        item.set_child(Some(&content));
    });
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(content), Some(entry)) = (
            item.child().and_downcast::<gtk::Box>(),
            item.item().and_downcast::<glib::BoxedAnyObject>(),
        ) else {
            return;
        };
        while let Some(child) = content.first_child() {
            content.remove(&child);
        }
        let entry = entry.borrow::<Entry>();
        let row = match &entry.shown {
            Some(shown) if !in_menu => shown,
            _ => &entry.row,
        };
        if row.kind == RowKind::Item {
            content.append(&icon::target_icon(&row.icon, row.badge.as_ref(), ICON_SIZE));
        }
        let label = gtk::Label::builder()
            .label(&row.label)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .hexpand(in_menu)
            .build();
        content.append(&label);
        if row.missing {
            let warning = gtk::Image::builder()
                .icon_name("dialog-warning-symbolic")
                .tooltip_text("This app is no longer installed")
                .css_classes(["warning"])
                .build();
            content.append(&warning);
        }
        if in_menu {
            // TGT-03. The list factory replaces AdwComboRow's own check, so
            // draw it here; hidden but sized on the other rows, as GTK's
            // drop-down does, so the labels keep their width.
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_opacity(if row.checked { 1.0 } else { 0.0 });
            content.append(&check);
        }
    });
    factory
}

/// A section's title in the dimmed heading style, or a plain line between
/// untitled sections (nothing above the first).
fn header_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_bind(|_, header| {
        let Some(header) = header.downcast_ref::<gtk::ListHeader>() else {
            return;
        };
        let title = header
            .item()
            .and_downcast::<glib::BoxedAnyObject>()
            .and_then(|entry| entry.borrow::<Entry>().header.clone())
            .unwrap_or_default();
        let child: Option<gtk::Widget> = if !title.is_empty() {
            Some(
                gtk::Label::builder()
                    .label(&title)
                    .xalign(0.0)
                    .css_classes(["dimmed", "wye-target-menu-header"])
                    .build()
                    .upcast(),
            )
        } else if header.start() > 0 {
            Some(gtk::Separator::new(gtk::Orientation::Horizontal).upcast())
        } else {
            None
        };
        header.set_child(child.as_ref());
    });
    factory
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, label: &str) -> Row {
        Row {
            kind,
            label: label.to_owned(),
            icon: String::new(),
            badge: None,
            target: Value::Null,
            checked: false,
            missing: false,
        }
    }

    #[test]
    fn tgt_02_separators_and_headers_become_sections() {
        let rows = [
            row(RowKind::Item, "Picker"),
            row(RowKind::Separator, ""),
            row(RowKind::Item, "Firefox"),
            row(RowKind::Item, "Konsole"),
            row(RowKind::Separator, ""),
            row(RowKind::Header, "Private Browsing"),
            row(RowKind::Item, "Firefox (Private)"),
            row(RowKind::Separator, ""),
            row(RowKind::Other, "Other…"),
        ];
        let entries = entries(&rows);
        let shape: Vec<(&str, Option<&str>)> = entries
            .iter()
            .map(|entry| (entry.row.label.as_str(), entry.header.as_deref()))
            .collect();
        assert_eq!(
            shape,
            [
                ("Picker", Some("")),
                ("Firefox", Some("")),
                ("Konsole", None),
                ("Firefox (Private)", Some("Private Browsing")),
                ("Other…", Some("")),
            ]
        );
    }
}
