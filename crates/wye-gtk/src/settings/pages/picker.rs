//! The Picker page (07-picker-settings.md): the picker's appearance, when it
//! is skipped, its keys, and Preview Picker. PKS-01 to PKS-09.
//!
//! The Picker Keys sheet (KEY-20 to KEY-22) is [`keys_sheet`]; its clash
//! rules and patches are wye-ui's Qt-free [`keys`].
//!
//! KDE counterpart: crates/wye-ui/qml/settings/PickerPage.qml.

// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
// The file is a symlink, so its nested `mod tests;` is found in `picker/keys/tests.rs`.
#[allow(
    dead_code,
    reason = "the file is shared whole; the sheet uses only part of it"
)]
mod keys;
mod keys_sheet;

use std::cell::OnceCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use super::{Context, Page};
use crate::settings::store::SettingsStore;
// What `keys` reads the shown list with (`super::shown`).
use crate::settings::shown;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::choice_row::ChoiceRow;
use crate::widgets::radio_row::RadioRow;
use crate::widgets::{group, switch_row};
use keys_sheet::KeysSheet;

/// PKS-01, PICK-11.
const ICON_SIZES: [(&str, &str); 3] =
    [("small", "Small"), ("medium", "Medium"), ("large", "Large")];

/// PKS-08, KEY-10: the hotkey schemes, as the picker names them.
const HOTKEY_SCHEMES: [(&str, &str); 4] = [
    ("per-target", "Assigned per browser"),
    ("numbers", "Numbers 1–9"),
    ("letters", "Letters from names"),
    ("off", "Off"),
];

/// The sheet `open_sheet` and Customize… open (KEY-20).
const KEYS_SHEET: &str = "picker-keys";

/// The Picker page.
#[derive(Debug)]
pub struct PickerPage {
    page: adw::PreferencesPage,
    context: Context,
    /// The Picker Keys sheet, built the first time it opens.
    sheet: Rc<OnceCell<KeysSheet>>,
}

impl PickerPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("Picker")
            .name("picker")
            .build();
        page.add(&appearance(store));
        page.add(&behaviour(store));

        // PKS-08, PKS-09
        let keys = group::group("Keys");
        let hotkeys = ChoiceRow::new("Target hotkeys", "", &HOTKEY_SCHEMES);
        hotkeys.bind(store, "picker.hotkeys", "per-target");
        keys.add(hotkeys.row());
        let customize = ButtonRow::new("Picker keys", "", "Customize…");
        keys.add(customize.row());
        page.add(&keys);

        // PKS-06: a card of its own below the others. Choosing a target in
        // the preview opens nothing; the service does that.
        let preview = group::group("");
        let preview_row = ButtonRow::new(
            "Preview",
            "See the picker as it looks now. Choosing a target in it opens nothing.",
            "Preview Picker",
        );
        let preview_content = adw::ButtonContent::builder()
            .icon_name("view-reveal-symbolic")
            .label("Preview Picker")
            .build();
        preview_row.button().set_child(Some(&preview_content));
        preview_row
            .button()
            .update_property(&[gtk::accessible::Property::Label("Preview Picker")]);
        preview_row.button().connect_clicked(glib::clone!(
            #[weak]
            store,
            move |_| store.call(|proxy| async move { proxy.preview_picker().await })
        ));
        preview.add(preview_row.row());
        page.add(&preview);

        let this = Self {
            page,
            context: context.clone(),
            sheet: Rc::default(),
        };
        let (context, sheet) = (this.context.clone(), Rc::clone(&this.sheet));
        customize
            .button()
            .connect_clicked(move |_| open_keys(&context, &sheet));
        // A sheet left open belongs to this page: it goes when the page does
        // (a request that switches pages, the window hidden).
        let sheet = Rc::clone(&this.sheet);
        this.page.connect_unmap(move |_| {
            if let Some(sheet) = sheet.get() {
                sheet.close();
            }
        });
        this
    }
}

impl Page for PickerPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn open_sheet(&self, name: &str) -> bool {
        if name != KEYS_SHEET {
            return false;
        }
        open_keys(&self.context, &self.sheet);
        true
    }
}

/// PKS-01 to PKS-04.
fn appearance(store: &SettingsStore) -> adw::PreferencesGroup {
    let appearance = group::group("Appearance");
    let icon_size = RadioRow::new("Icon size", "", &ICON_SIZES);
    icon_size.bind(store, "picker.icon-size", "large");
    appearance.add(icon_size.row());
    for (title, path, default) in [
        ("Show browser names", "picker.show-names", true),
        ("Show URL", "picker.show-url", false),
        ("Show profile badge", "picker.show-profile-badge", true),
    ] {
        let row = switch_row::switch_row(title, "");
        switch_row::bind(store, &row, path, default);
        appearance.add(&row);
    }
    appearance
}

/// PKS-05; PKS-07 (a link that needs the picker waits for the unlock) is
/// the service's.
fn behaviour(store: &SettingsStore) -> adw::PreferencesGroup {
    let behaviour = group::group("Behaviour");
    let skip = switch_row::switch_row(
        "Skip picker when screen is locked",
        "Links will then open in the alternative browser.",
    );
    switch_row::bind(store, &skip, "picker.skip-when-locked", false);
    behaviour.add(&skip);
    behaviour
}

/// Open the Picker Keys sheet over the window (KEY-20).
fn open_keys(context: &Context, sheet: &OnceCell<KeysSheet>) {
    if let Some(window) = context.window() {
        sheet
            .get_or_init(|| KeysSheet::new(&context.store))
            .present(&window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pks_01_icon_sizes_in_order_with_large_known() {
        let values: Vec<&str> = ICON_SIZES.iter().map(|(value, _)| *value).collect();
        assert_eq!(values, ["small", "medium", "large"]);
    }

    #[test]
    fn key_10_hotkey_schemes_match_the_configuration() {
        for (value, _) in HOTKEY_SCHEMES {
            let parsed: Result<wye_core::config::HotkeyScheme, _> =
                serde_json::from_value(serde_json::Value::from(value));
            assert!(parsed.is_ok(), "{value} is not a hotkey scheme");
        }
        assert_eq!(HOTKEY_SCHEMES[0], ("per-target", "Assigned per browser"));
    }
}
