//! BLK-03 Switch row: an `AdwSwitchRow`, the row with an on/off switch as
//! its trailing control (the whole row toggles it).
//!
//! API:
//! - [`switch_row`]`(title, subtitle)`: the row.
//! - [`bind`]`(store, row, path, default)`: show the boolean at `path`
//!   (`general.show-tray-icon`), save every flip (SET-06), and follow
//!   `writable`. For a row whose value comes from elsewhere (GEN-01's Nix
//!   managed login start) set it in a `store.connect_changed` callback
//!   instead.

use gtk::glib;
use serde_json::Value;

use super::row;
use crate::settings::store::SettingsStore;

/// A switch row titled `title`, with `subtitle` unless empty.
#[must_use]
pub fn switch_row(title: &str, subtitle: &str) -> adw::SwitchRow {
    row::titled(adw::SwitchRow::new(), title, subtitle)
}

/// Tie `row` to the boolean at `path`, `default` when the file does not set
/// it. See the module docs.
pub fn bind(store: &SettingsStore, row: &adw::SwitchRow, path: &'static str, default: bool) {
    let show = glib::clone!(
        #[weak]
        row,
        move |store: &SettingsStore| {
            let on = store.bool_value(path, default);
            if row.is_active() != on {
                row.set_active(on);
            }
        }
    );
    show(store);
    store.connect_changed_while(row, show);
    row.connect_active_notify(glib::clone!(
        #[weak]
        store,
        move |row| {
            if row.is_active() != store.bool_value(path, default) {
                store.set_value(path, Value::Bool(row.is_active()));
            }
        }
    ));
    row::follow_writable(store, row, |_| true);
}
