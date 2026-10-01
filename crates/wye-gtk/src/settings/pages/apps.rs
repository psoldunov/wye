//! The Apps page (06-apps.md): routes links to well-known web services. The
//! "Please Read" callout, then one card titled with the page's heading, a
//! target row per service, sorted by name. APP-01 to APP-10.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/AppsPage.qml.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::{Context, Page};
use crate::settings::menu::Surface;
use crate::settings::sheets::app_chooser::{AppChooser, Choice};
use crate::settings::store::SettingsStore;
use crate::widgets::callout::Callout;
use crate::widgets::target_row::TargetRow;
use crate::widgets::{empty_state, group};

/// APP-01, wording as on KDE.
const READ_FIRST: &str = "This lets you open links <b>to</b> certain websites directly in their desktop app or in a specific browser. To open links <b>clicked in</b> a certain app in a specific browser, create a custom rule with “Source Apps” matching.";

/// APP-02: the card's title.
const HEADING: &str = "Open links to web apps in their desktop app or a specific browser";

/// One service as the rows show it: ID, name and installed app. The rows are
/// rebuilt only when this changes, not on every mapping, so a menu or the
/// chooser a row has open stays (KDE's `catalogueKey`).
type Catalogue = Vec<(String, String, String)>;

/// The Apps page.
#[derive(Debug)]
pub struct AppsPage {
    page: adw::PreferencesPage,
    state: Rc<State>,
}

#[derive(Debug)]
struct State {
    context: Context,
    services: adw::PreferencesGroup,
    empty: adw::PreferencesGroup,
    rows: RefCell<Vec<(String, TargetRow)>>,
    catalogue: RefCell<Option<Catalogue>>,
}

impl AppsPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("Apps")
            .name("apps")
            .build();

        // APP-01
        let callout = Callout::new(store, "apps-read-first", "Please Read", READ_FIRST);
        page.add(callout.group());

        // APP-02, APP-03
        let services = group::group(HEADING);
        page.add(&services);
        let empty = group::group("");
        empty.add(&empty_state::empty_state(
            "wye-apps-symbolic",
            "No Web Apps",
            "Wye’s catalogue of web apps is empty.",
        ));
        page.add(&empty);

        let state = Rc::new(State {
            context: context.clone(),
            services,
            empty,
            rows: RefCell::default(),
            catalogue: RefCell::default(),
        });
        show(&state, store);
        let weak = Rc::downgrade(&state);
        store.connect_changed(move |store| {
            if let Some(state) = weak.upgrade() {
                show(&state, store);
            }
        });
        Self { page, state }
    }
}

impl Page for AppsPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    /// `service-menu:<id>` (that service's target menu open),
    /// `app-chooser:<id>` ("Other…" of that service).
    fn open_sheet(&self, name: &str) -> bool {
        let state = &self.state;
        let (name, id) = name.split_once(':').unwrap_or((name, ""));
        let row = state
            .rows
            .borrow()
            .iter()
            .find(|(service, _)| service == id)
            .map(|(_, row)| row.clone());
        match (name, row) {
            ("service-menu", Some(row)) => {
                adw::prelude::ActionRowExt::activate(row.row());
            }
            ("app-chooser", Some(_)) => choose_other(state, id),
            _ => return false,
        }
        true
    }
}

/// Rebuild the rows when the catalogue changed; the rows follow their
/// mappings themselves (`TargetRow::bind_service`).
fn show(state: &Rc<State>, store: &SettingsStore) {
    let catalogue: Catalogue = store.with_snapshot(|s| {
        let mut services: Catalogue = s
            .services
            .services
            .iter()
            .map(|service| {
                let installed = service
                    .installed_app
                    .as_ref()
                    .map(|app| app.id.clone())
                    .unwrap_or_default();
                (service.id.clone(), service.name.clone(), installed)
            })
            .collect();
        // APP-03: alphabetical by service name.
        services.sort_by_key(|(_, name, _)| name.to_lowercase());
        services
    });
    let empty = catalogue.is_empty();
    state.services.set_visible(!empty);
    state.empty.set_visible(empty && store.loaded());
    if state.catalogue.borrow().as_ref() == Some(&catalogue) {
        return;
    }
    for (_, row) in state.rows.take() {
        state.services.remove(row.row());
    }
    let rows: Vec<(String, TargetRow)> = catalogue
        .iter()
        .map(|(id, name, _)| {
            // APP-04 to APP-06, APP-10: Default (<primary>) until mapped;
            // the service's own app right below Default; a missing app warns.
            let row = TargetRow::new(&gtk::glib::markup_escape_text(name), "", Surface::Apps);
            row.bind_service(store, id);
            let weak = Rc::downgrade(state);
            let service = id.clone();
            row.connect_other(move || {
                if let Some(state) = weak.upgrade() {
                    choose_other(&state, &service);
                }
            });
            state.services.add(row.row());
            (id.clone(), row)
        })
        .collect();
    state.rows.replace(rows);
    state.catalogue.replace(Some(catalogue));
}

/// TGT-06: "Other…" maps the service to any app.
fn choose_other(state: &State, service: &str) {
    let Some(window) = state.context.window() else {
        return;
    };
    let store = state.context.store.downgrade();
    let service = service.to_owned();
    AppChooser::open(
        &window,
        &state.context.store,
        Choice::Single,
        move |targets| {
            if let (Some(store), Some(target)) = (store.upgrade(), targets.first()) {
                store.set_service_target(&service, target);
            }
        },
    );
}
