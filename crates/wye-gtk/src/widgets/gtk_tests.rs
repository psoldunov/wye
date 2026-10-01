//! The kit under a real GTK: a value the store pushes is never written back
//! (SET-06, BLK-04, BLK-06), rows built again leave no handler on the store,
//! and the checklist keeps the keyboard focus on its row when the list is
//! built again (BLK-15).
//!
//! GTK needs a display and lives on the thread that initialised it, so the
//! test runs itself again in a child on a private Xvfb (as `--self-test`
//! does), with every check in that one child. Without `Xvfb` on `PATH` (the
//! Nix build sandbox) it is skipped.

use std::process::Command;

use adw::prelude::*;
use gtk::glib;
use serde_json::json;

use super::checklist::{Checklist, ChecklistItem};
use super::choice_row::ChoiceRow;
use super::radio_row::RadioRow;
use super::target_row::TargetRow;
use super::{row, switch_row};
use crate::selftest::display::Display;
use crate::settings::menu::Surface;
use crate::settings::store::SettingsStore;
use crate::settings::store::tests::store_with;

/// Set in the child that runs the checks.
const CHILD: &str = "WYE_GTK_KIT_TEST";
/// This test, as libtest names it.
const NAME: &str = "widgets::gtk_tests::kit_under_gtk";

#[test]
fn kit_under_gtk() {
    if std::env::var_os(CHILD).is_some() {
        gtk::init().expect("GTK on the private display");
        adw::init().expect("libadwaita");
        an_unknown_value_is_never_written_back();
        rows_built_again_leave_no_handler();
        the_checklist_keeps_the_focus_on_its_row();
        return;
    }
    let Some(display) = Display::start() else {
        eprintln!("skipped: no Xvfb to run GTK on");
        return;
    };
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args([NAME, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("DISPLAY", display.name())
        .env("GDK_BACKEND", "x11")
        .env("GSETTINGS_BACKEND", "memory")
        .env("NO_AT_BRIDGE", "1")
        // No GPU on Xvfb: the cairo renderer, as the self-test uses.
        .env("GSK_RENDERER", "cairo")
        .status()
        .expect("the child runs");
    assert!(status.success(), "the GTK checks failed: {status}");
}

/// SET-06: a value a newer Wye wrote shows as the default and stays in the
/// file until the user chooses.
fn an_unknown_value_is_never_written_back() {
    let store = store_with(&json!({
        "general": {"tray-icon": "newer"},
        "picker": {"icon-size": "huge"}
    }));
    let choice = ChoiceRow::new(
        "Tray icon",
        "",
        &[("primary-browser", "Primary Browser"), ("wye", "Wye")],
    );
    choice.bind(&store, "general.tray-icon", "primary-browser");
    let radio = RadioRow::new("Icon size", "", &[("small", "Small"), ("large", "Large")]);
    radio.bind(&store, "picker.icon-size", "large");
    // Another change makes both show the store again.
    store.set_value("general.show-tray-icon", json!(false));
    assert_eq!(store.string_value("general.tray-icon", ""), "newer");
    assert_eq!(store.string_value("picker.icon-size", ""), "huge");
    assert_eq!(choice.row().selected(), 0, "shown as the default");
    assert_eq!(radio.toggles().active_name().as_deref(), Some("large"));
    // The user's choice is saved.
    choice.row().set_selected(1);
    radio.toggles().set_active_name(Some("small"));
    assert_eq!(store.string_value("general.tray-icon", ""), "wye");
    assert_eq!(store.string_value("picker.icon-size", ""), "small");
}

/// Whether anything still follows `store`'s `changed`.
fn followed(store: &SettingsStore) -> bool {
    let changed = glib::subclass::SignalId::lookup("changed", SettingsStore::static_type())
        .expect("the store's signal");
    glib::signal::signal_has_handler_pending(store, changed, None, true)
}

/// Rows a page builds again (the Apps page's services, the expansion
/// sheet's domains) must not stay connected to the store for its life.
fn rows_built_again_leave_no_handler() {
    let store = store_with(&json!({}));
    for _ in 0..3 {
        let switch = switch_row::switch_row("Switch", "");
        switch_row::bind(&store, &switch, "general.show-tray-icon", true);
        let target = TargetRow::new("Browser", "", Surface::Browsers);
        target.bind(&store, "browsers.primary", &json!({"picker": true}));
        let label = gtk::Label::new(None);
        row::follow_writable(&store, &label, |_| true);
        assert!(followed(&store));
        drop((switch, target, label));
        store.set_value("general.show-tray-icon", json!(false));
        assert!(!followed(&store), "a dropped row still follows the store");
    }
}

fn item(key: &str, checked: bool) -> ChecklistItem {
    ChecklistItem {
        key: key.to_owned(),
        title: key.to_uppercase(),
        subtitle: String::new(),
        icon: String::new(),
        badge: None,
        checked,
        extra: None,
    }
}

/// BLK-15: the list is built again after every change; the focus stays on
/// the row it was on, wherever that row lands.
fn the_checklist_keeps_the_focus_on_its_row() {
    let checklist = Checklist::new();
    let window = gtk::Window::builder().child(checklist.widget()).build();
    checklist.set_items(&[item("a", true), item("b", true), item("c", false)]);
    window.present();
    while glib::MainContext::default().iteration(false) {}
    let check = checklist.check_of("b").expect("row b");
    check.grab_focus();
    // Moved up (Alt+Up), then unchecked: rebuilt both times.
    checklist.set_items(&[item("b", true), item("a", true), item("c", false)]);
    let focus = gtk::prelude::GtkWindowExt::focus(&window);
    assert_eq!(
        focus.as_ref(),
        checklist.check_of("b").map(Cast::upcast).as_ref()
    );
    checklist.set_items(&[item("a", true), item("b", false), item("c", false)]);
    let focus = gtk::prelude::GtkWindowExt::focus(&window);
    assert_eq!(
        focus.as_ref(),
        checklist.check_of("b").map(Cast::upcast).as_ref()
    );
    assert_ne!(
        checklist.check_of("b"),
        Some(check),
        "the row was built again"
    );
    window.destroy();
}
