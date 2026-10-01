//! The Rules page's parts (08-rules.md, 17-dialogs.md "Rule tester"): the
//! rule list, the rule editor, the rule tester, the rules help, the
//! source-app chooser and import/export. The page itself is
//! `crate::settings::pages::rules`.
//!
//! GTK-free, compiled from wye-ui's Qt-free model (one source for both
//! frontends; crates/wye-ui owns it):
//!
//! - [`list`]: the rows and their one-line summaries (RUL-07).
//! - [`ops`]: move, toggle, delete, undo, duplicate, save, each a new list
//!   and the merge patch that stores it (RUL-02 to RUL-28).
//! - [`draft`]: the editor's draft, its prefill and validity (RUL-10 to
//!   RUL-28, PICK-31).
//! - [`tester`]: `TestLink`'s trace as rows, and the context it sends
//!   (DLG-TST-01, DLG-TST-02).
//!
//! wye-ui's `rules/files.rs` is not shared: `GtkFileDialog` hands over a
//! `GFile`, read and written with GIO ([`transfer`]).
//!
//! On GTK:
//!
//! - [`rule_list`]: the list card's rows (RUL-04 to RUL-07).
//! - [`editor`] with [`matcher_row`]: the rule editor sheet.
//! - [`tester_sheet`] with [`trace_rows`]: the rule tester sheet.
//! - [`help`]: the rules help dialog (RUL-19).
//! - [`transfer`]: Import Rules… and Export Rules… (RUL-02).
//! - [`script`]: opening a rule's transform script (RUL-25).

#[path = "../../../wye-ui/src/rules/draft.rs"]
pub mod draft;
pub mod editor;
pub mod help;
// `app_names` reads apps as JSON; the GTK page has them typed (`Apps::names`).
#[allow(
    dead_code,
    reason = "the file is shared whole; the GTK page uses only part of it"
)]
#[path = "../../../wye-ui/src/rules/list.rs"]
pub mod list;
pub mod matcher_row;
// Compiled from wye-ui's Qt-free model; `all_removed` is the one part the GTK
// page words differently (its own undo for Delete All Rules…).
#[allow(
    dead_code,
    reason = "the file is shared whole; the GTK page uses only part of it"
)]
#[path = "../../../wye-ui/src/rules/ops.rs"]
pub mod ops;
pub mod rule_list;
pub mod script;
#[path = "../../../wye-ui/src/rules/tester.rs"]
pub mod tester;
pub mod tester_sheet;
pub mod trace_rows;
pub mod transfer;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use wye_api::apps::AppList;
use wye_core::{Modifiers, Rule};

use crate::settings::store::SettingsStore;

/// The rules the store holds, or `None` (logged) when they cannot be read:
/// a change built on an empty list instead would delete every rule.
#[must_use]
pub fn rules(store: &SettingsStore) -> Option<Vec<Rule>> {
    let config = store.with_snapshot(|snapshot| snapshot.config.to_string());
    list::rules_of(&config)
        .inspect_err(|error| tracing::warn!(%error, "cannot change the rules"))
        .ok()
}

/// Save `rules` as the whole list (an array: the patch carries all of it,
/// SET-06).
pub fn save(store: &SettingsStore, rules: &[Rule]) {
    match ops::patch(rules) {
        Ok(patch) => store.apply_patch(&patch),
        Err(error) => tracing::warn!(%error, "cannot write the rules"),
    }
}

/// `browsers.alternative-key`, which wins over a rule's held keys (RUL-27).
#[must_use]
pub fn alternative_key(store: &SettingsStore) -> Modifiers {
    store.with_snapshot(|snapshot| snapshot.typed_config().browsers.alternative_key)
}

/// A seed for new rule IDs ([`draft::fresh_id`]): the time in milliseconds.
#[must_use]
pub fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| {
            u64::try_from(time.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The installed apps (`GetApps(true)`), read once per window: names and
/// icons for source apps (RUL-07, RUL-16) and the tester's Source app
/// popup (DLG-TST-01). Clones share the list.
#[derive(Debug, Clone, Default)]
pub struct Apps {
    list: Rc<RefCell<AppList>>,
    /// Asked for, or read: no second request.
    asked: Rc<Cell<bool>>,
}

impl Apps {
    /// Read the apps through `store` (the fixture's under the self-test)
    /// unless they were read already, and run `loaded` once they are in. A
    /// failed read is tried again on the next call.
    pub fn ensure(&self, store: &SettingsStore, loaded: impl Fn() + 'static) {
        if !store.loaded() || self.asked.replace(true) {
            return;
        }
        let (list, asked) = (Rc::clone(&self.list), Rc::clone(&self.asked));
        store.apps(move |result| match result {
            Ok(apps) => {
                list.replace(apps);
                loaded();
            }
            Err(error) => {
                asked.set(false);
                tracing::info!(%error, "no apps for the rule summaries");
            }
        });
    }

    /// The list as `GetApps` returns it, for [`list::source_rows`].
    #[must_use]
    pub fn json(&self) -> String {
        serde_json::to_string(&*self.list.borrow()).unwrap_or_default()
    }

    /// Name by desktop ID, for [`list::rows`].
    #[must_use]
    pub fn names(&self) -> list::AppNames {
        self.list
            .borrow()
            .apps
            .iter()
            .map(|app| (app.id.clone(), app.name.clone()))
            .collect()
    }

    /// A copy of the list.
    #[must_use]
    pub fn get(&self) -> AppList {
        self.list.borrow().clone()
    }
}
