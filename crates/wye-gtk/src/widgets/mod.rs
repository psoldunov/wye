//! The shared building blocks of the Settings window and the other windows
//! (03-settings-window.md, "Shared building blocks"), mapped to libadwaita
//! as its "Native control mapping" table says. Pages are built from these
//! and from plain libadwaita rows; a page never styles a widget itself.
//!
//! | Spec | Block | Here |
//! |---|---|---|
//! | BLK-01 | Group card | [`group::group`] (`AdwPreferencesGroup`) |
//! | BLK-02 | Row | [`row::action_row`], [`row::add_leading_icon`] |
//! | BLK-03 | Switch row | [`switch_row::switch_row`], [`switch_row::bind`] |
//! | BLK-04 | Target popup row | [`target_row::TargetRow`] (`AdwComboRow`, sections, icons) |
//! | BLK-05 | Button row | [`button_row::ButtonRow`] |
//! | BLK-06 | Inline radio row | [`radio_row::RadioRow`] (`AdwToggleGroup`) |
//! | — | Choice popup row | [`choice_row::ChoiceRow`] (`AdwComboRow` of fixed values, GEN-02) |
//! | BLK-07 | Text entry row | [`entry_row::EntryRow`] (`AdwEntryRow`) |
//! | BLK-08 | Help button | [`help::help_button`], [`help::add_help`] |
//! | BLK-09 | Callout | [`callout::Callout`] |
//! | BLK-10 | Disabled row | [`row::set_disabled`] |
//! | BLK-11 | Sheet | [`sheet::Sheet`] (`AdwDialog`) |
//! | BLK-12 | Empty state | [`empty_state::empty_state`] (`AdwStatusPage`) |
//! | BLK-13 | List toolbar | [`list_toolbar::ListCard`] |
//! | BLK-14 | Section with add button | [`section::AddSection`] |
//! | BLK-15 | Reorderable checklist | [`checklist::Checklist`] |
//! | BLK-16 | Shortcut recorder | [`shortcut::ShortcutRecorder`], [`shortcut::ShortcutChips`] |
//! | BLK-17 | Inline links | [`links::route_links`] |
//! | BLK-18 | Modifier chooser | [`modifiers::ModifierChooser`] |
//! | — | Toast | [`toast::show`] (`AdwToast`, the undo of RUL-06) |
//! | — | Target icons in menus | [`menu_icons::add`] (TRAY-08, PICK-28) |
//! | — | Page column | [`page::widen`] (one width for every Settings page) |
//!
//! Conventions every block follows:
//!
//! - A constructor returns the libadwaita widget, or a small struct whose
//!   `widget()`/`row()` is what goes into a group; nothing is added to a
//!   parent for the caller.
//! - A control that edits the configuration has a `bind(store, path, …)`
//!   that shows the value at `path` now and after every store change, saves
//!   a change through the [`SettingsStore`](crate::settings::store::SettingsStore)
//!   (SET-06), and follows `writable` (a read-only file disables it). A
//!   change the store pushes back is never saved again: binds compare with
//!   the store before saving.
//! - Callbacks capture widgets weakly (`glib::clone!(#[weak] …)`); the store
//!   outlives every widget of its window.

pub mod button_row;
pub mod callout;
pub mod checklist;
pub mod choice_row;
pub mod empty_state;
pub mod entry_row;
pub mod group;
pub mod help;
pub mod icon;
pub mod links;
pub mod list_toolbar;
pub mod menu_icons;
pub mod modifiers;
pub mod page;
pub mod radio_row;
pub mod row;
pub mod section;
pub mod sheet;
pub mod shortcut;
pub mod switch_row;
pub mod target_row;
pub mod toast;

#[cfg(test)]
mod gtk_tests;
