//! The Picker Keys sheet (15-keyboard.md, KEY-20 to KEY-22): every picker
//! action with its bindings as removable chips and a "+" that records
//! another (KEY-02), the three held-modifier actions with modifier choosers
//! (KEY-01), and Reset to Defaults (KEY-04). Changes apply at once; Done
//! closes the sheet.
//!
//! A key belongs to one action and cannot also be a target hotkey; the three
//! held-modifier actions need different sets. On a clash the sheet asks
//! "Already used by …" with **Replace** (KEY-21), as GNOME Settings does for
//! its keyboard shortcuts.
//!
//! KDE counterpart: crates/wye-ui/qml/settings/PickerKeysSheet.qml.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;
use wye_core::keybinding::{KeyBinding, canonical_key};
use wye_core::{Modifier, Modifiers};

use super::keys::{self, Check, Clash, HotkeyUse};
use crate::settings::snapshot::Snapshot;
use crate::settings::store::SettingsStore;
use crate::widgets::group;
use crate::widgets::help;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::row;
use crate::widgets::sheet::Sheet;
use crate::widgets::shortcut::ShortcutChips;

/// The subtitle of a chooser the session cannot serve (KEY-06).
const UNAVAILABLE: &str = "Not available in this session";

/// The Picker Keys sheet. It is built once per window and presented again
/// each time: it follows the store while closed, so it opens current.
#[derive(Debug, Clone)]
pub struct KeysSheet {
    sheet: Sheet,
    /// Presented and not closed yet.
    open: Rc<Cell<bool>>,
}

impl KeysSheet {
    /// The sheet on `store`, built and kept current; show it with
    /// [`Self::present`].
    #[must_use]
    pub fn new(store: &SettingsStore) -> Self {
        let sheet = Sheet::new("Picker Keys", "Done");
        // KEY-20: changes apply at once, so there is nothing to cancel.
        sheet.cancel().set_visible(false);
        sheet.set_valid(true);
        sheet.connect_primary(|| true);
        let page = sheet.page();
        page.add(&actions(store));
        page.add(&holds(store));
        page.add(&reset(store));
        let open = Rc::new(Cell::new(false));
        let closed = Rc::clone(&open);
        sheet.dialog().connect_closed(move |_| closed.set(false));
        Self { sheet, open }
    }

    /// Show the sheet over the window `parent` belongs to, from the top.
    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.close();
        self.sheet.page().scroll_to_top();
        self.sheet.present(parent);
        self.open.set(true);
    }

    /// Close the sheet if it is open.
    pub fn close(&self) {
        if self.open.replace(false) {
            self.sheet.dialog().force_close();
        }
    }
}

/// KEY-20, KEY-22: one row per picker action, its bindings as chips.
fn actions(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("Actions");
    for (action, key) in keys::ACTION_KEYS {
        let row = row::action_row(action.label(), "");
        row.add_css_class("wye-key-row");
        let chips = ShortcutChips::new();
        chips.widget().set_halign(gtk::Align::End);
        row.add_suffix(chips.widget());
        group.add(&row);

        let shown: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
        let show = glib::clone!(
            #[strong]
            chips,
            move |store: &SettingsStore| {
                let stored = bindings(store, key);
                if shown.borrow().as_ref() != Some(&stored) {
                    chips.set_bindings(&stored);
                    shown.replace(Some(stored));
                }
            }
        );
        show(store);
        store.connect_changed(show);
        chips.connect_added(glib::clone!(
            #[weak]
            store,
            #[weak]
            row,
            move |stored| try_key(&store, &row, key, stored)
        ));
        chips.connect_removed(glib::clone!(
            #[weak]
            store,
            move |index| {
                let mut kept = bindings(&store, key);
                if index < kept.len() {
                    kept.remove(index);
                    let list = kept.into_iter().map(Value::String).collect();
                    store.set_value(&format!("picker.keys.{key}"), Value::Array(list));
                }
            }
        ));
        row::follow_writable(store, chips.widget(), |_| true);
    }
    group
}

/// KEY-13, KEY-20: the held-modifier actions.
fn holds(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("Hold while choosing");
    for (key, title) in keys::MODIFIER_ACTIONS {
        let chooser = ModifierChooser::new();
        let row = chooser.add_row(title, "");
        let help = help::add_help(store, &row, "held-keys-unavailable");
        group.add(&row);

        let show = glib::clone!(
            #[strong]
            chooser,
            #[weak]
            row,
            move |store: &SettingsStore| {
                chooser.set_pressed(&modifier_names(store, key));
                // KEY-06: where held keys are not reported, say so.
                let available = store.with_snapshot(Snapshot::held_keys_available);
                row::set_unavailable(&row, !available);
                row.set_subtitle(if available { "" } else { UNAVAILABLE });
                help.set_visible(!available);
            }
        );
        show(store);
        store.connect_changed(show.clone());
        chooser.connect_changed(glib::clone!(
            #[weak]
            store,
            #[weak]
            row,
            move |names| try_modifiers(&store, &row, key, names, &show)
        ));
        row::follow_writable(store, chooser.widget(), |store| {
            store.with_snapshot(Snapshot::held_keys_available)
        });
    }
    group
}

/// KEY-04: the whole `picker.keys` section back to its defaults.
fn reset(store: &SettingsStore) -> adw::PreferencesGroup {
    let group = group::group("");
    let button = adw::ButtonRow::builder()
        .title("Reset to Defaults")
        .start_icon_name("edit-undo-symbolic")
        .build();
    button.connect_activated(glib::clone!(
        #[weak]
        store,
        move |_| store.reset_section("picker.keys")
    ));
    row::follow_writable(store, &button, |_| true);
    group.add(&button);
    group
}

/// The bindings of action `key` now (the defaults when the file sets none).
fn bindings(store: &SettingsStore, key: &str) -> Vec<String> {
    store.with_snapshot(|s| {
        let keys = s.typed_config().picker.keys;
        match key {
            "open" => keys.open,
            "cancel" => keys.cancel,
            "next" => keys.next,
            "previous" => keys.previous,
            "first" => keys.first,
            "last" => keys.last,
            "copy-link" => keys.copy_link,
            "more" => keys.more,
            "create-rule" => keys.create_rule,
            _ => Vec::new(),
        }
    })
}

/// The modifiers of the held-modifier action `key` now, by name.
fn modifier_names(store: &SettingsStore, key: &str) -> Vec<String> {
    let set = store.with_snapshot(|s| {
        let keys = s.typed_config().picker.keys;
        match key {
            "private-modifier" => keys.private_modifier,
            "background-modifier" => keys.background_modifier,
            "new-window-modifier" => keys.new_window_modifier,
            _ => Modifiers::NONE,
        }
    });
    set.iter()
        .map(|modifier| modifier.name().to_owned())
        .collect()
}

/// The target hotkeys in use, each with its target's name, for KEY-12.
fn hotkey_uses(store: &SettingsStore) -> Vec<HotkeyUse> {
    let entries = store.shown_entries();
    store.with_snapshot(|s| {
        let names: HashMap<String, &str> = s
            .targets
            .targets
            .iter()
            .map(|info| (info.target.to_string(), info.name.as_str()))
            .collect();
        let mut uses = Vec::new();
        for entry in &entries {
            let Some(Ok(key)) = entry.hotkey.as_deref().map(canonical_key) else {
                continue;
            };
            let target = entry.target.to_string();
            let name = names
                .get(&target)
                .map_or(target.clone(), |name| (*name).to_owned());
            uses.push(HotkeyUse { key, name });
        }
        uses
    })
}

/// KEY-02, KEY-21: give action `key` the recorded `stored` binding, or ask.
fn try_key(store: &SettingsStore, parent: &adw::ActionRow, key: &'static str, stored: &str) {
    let binding = match keys::parse(stored) {
        Ok(binding) => binding,
        Err(message) => {
            store.report(&message);
            return;
        }
    };
    let uses = hotkey_uses(store);
    let checked =
        store.with_snapshot(|s| keys::check_binding(&s.typed_config(), &uses, key, &binding));
    match checked {
        Ok(Check::Free) => bind(store, key, &binding, false),
        Ok(Check::Duplicate) => {}
        Ok(Check::Clash(clash)) => {
            let body = format!(
                "Replace it to use {} for “{}” instead.",
                binding.label(),
                parent.title()
            );
            ask_replace(
                parent,
                &clash,
                &body,
                glib::clone!(
                    #[weak]
                    store,
                    move |replace| {
                        if replace {
                            bind(&store, key, &binding, true);
                        }
                    }
                ),
            );
        }
        Err(message) => store.report(&message),
    }
}

/// Save `binding` for action `key`; with `replace` it leaves whatever had
/// it (KEY-21).
fn bind(store: &SettingsStore, key: &str, binding: &KeyBinding, replace: bool) {
    let entries = store.shown_entries();
    let built = store
        .with_snapshot(|s| keys::bind_patch(&s.typed_config(), &entries, key, binding, replace));
    match built {
        Ok(patch) => store.apply_patch(&patch),
        Err(message) => store.report(&message),
    }
}

/// KEY-01, KEY-21: give the held-modifier action `key` the pressed `names`,
/// or ask; a refused change shows the stored set again (`show`).
fn try_modifiers(
    store: &SettingsStore,
    parent: &adw::ActionRow,
    key: &'static str,
    names: &[String],
    show: &(impl Fn(&SettingsStore) + Clone + 'static),
) {
    let parsed: Result<Vec<Modifier>, _> = names.iter().map(|name| name.parse()).collect();
    let Ok(pressed) = parsed else {
        show(store);
        return;
    };
    let set = Modifiers::from_slice(&pressed);
    let save = move |store: &SettingsStore, replace: bool| {
        let built =
            store.with_snapshot(|s| keys::modifiers_patch(&s.typed_config(), key, set, replace));
        match built {
            Ok(patch) => store.apply_patch(&patch),
            Err(message) => store.report(&message),
        }
    };
    match store.with_snapshot(|s| keys::check_modifiers(&s.typed_config(), key, set)) {
        Ok(None) => save(store, false),
        Ok(Some(clash)) => {
            let body = format!("Replace it to hold {set} for “{}” instead.", parent.title());
            let show = show.clone();
            ask_replace(
                parent,
                &clash,
                &body,
                glib::clone!(
                    #[weak]
                    store,
                    move |replace| {
                        if replace {
                            save(&store, true);
                        } else {
                            show(&store);
                        }
                    }
                ),
            );
        }
        Err(message) => {
            store.report(&message);
            show(store);
        }
    }
}

/// KEY-21: "Already used by …" with Replace and Cancel; `answered(true)`
/// for Replace.
fn ask_replace(
    parent: &impl IsA<gtk::Widget>,
    clash: &Clash,
    body: &str,
    answered: impl Fn(bool) + 'static,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(clash.message())
        .body(body)
        .close_response("cancel")
        .default_response("replace")
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("replace", "Replace")]);
    dialog.set_response_appearance("replace", adw::ResponseAppearance::Suggested);
    dialog.connect_response(None, move |_, response| answered(response == "replace"));
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A writable store showing `config`, as the self-test's fixtures do.
    fn store_with(config: &Value) -> SettingsStore {
        let fixture = json!({
            "config": config,
            "revision": 3,
            "status": {"config": {"writable": true}},
        });
        let store = SettingsStore::new();
        assert!(store.load_fixture(&fixture).is_ok(), "a fixture");
        store
    }

    #[test]
    fn key_22_defaults_show_when_the_file_sets_none() {
        let store = store_with(&json!({}));
        assert_eq!(bindings(&store, "cancel"), ["Escape"]);
        assert_eq!(modifier_names(&store, "private-modifier"), ["Shift"]);
    }

    #[test]
    fn key_12_target_hotkeys_are_named_by_their_target() {
        let store = store_with(&json!({"browsers": {"shown": [
            {"target": {"app": "firefox.desktop"}, "hotkey": "F"},
            {"target": {"app": "chromium.desktop"}}
        ]}}));
        let uses = hotkey_uses(&store);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].key, "f");
    }

    #[test]
    fn key_21_a_binding_is_added_and_replace_takes_it_from_its_owner() {
        let store = store_with(&json!({}));
        let added = keys::parse("Ctrl+Shift+c").expect("a binding");
        bind(&store, "copy-link", &added, false);
        assert_eq!(bindings(&store, "copy-link"), ["Ctrl+c", "Ctrl+Shift+c"]);
        // Escape belongs to Cancel: Replace moves it.
        let escape = keys::parse("Escape").expect("a binding");
        bind(&store, "open", &escape, true);
        assert!(bindings(&store, "cancel").is_empty());
        assert!(bindings(&store, "open").contains(&"Escape".to_owned()));
    }
}
