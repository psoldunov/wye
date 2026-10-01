//! The picker's menus as `GMenu` models for a `GtkPopoverMenu`: the "⋯"
//! menu (PICK-08, PICK-28, PICK-31) and a tile's context menu (PICK-30),
//! with the entries crates/wye-ui's QML menus show. Their items name the
//! actions `super` adds to the picker window under [`GROUP`]; Open In's
//! icons are [`open_in_icons`], for `menu_icons::add` once the popover has
//! the model.

use gtk::gio;
use gtk::prelude::*;

use super::model;
use super::state::PickerState;
use crate::widgets::menu_icons::ItemIcon;

/// The action group of the picker window.
pub const GROUP: &str = "picker";
/// `picker.open-in("<group>:<item>")`: an Open In entry (PICK-28).
pub const OPEN_IN: &str = "open-in";
/// `picker.tile("<index>:<action>")`: a tile menu entry (PICK-30).
pub const TILE: &str = "tile";
/// The "⋯" menu's own actions: Copy Link, Create Rule… (PICK-31), Settings….
pub const COPY_LINK: &str = "copy-link";
pub const CREATE_RULE: &str = "create-rule";
pub const SETTINGS: &str = "settings";

fn detailed(action: &str) -> String {
    format!("{GROUP}.{action}")
}

fn item(label: &str, action: &str, target: Option<&str>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    match target {
        Some(target) => item.set_action_and_target_value(
            Some(&detailed(action)),
            Some(&gtk::glib::Variant::from(target)),
        ),
        None => item.set_detailed_action(&detailed(action)),
    }
    item
}

/// The "⋯" menu (PICK-08): Open In ›, then Copy Link and Create Rule…,
/// then Settings….
pub fn overflow(state: &PickerState) -> gio::Menu {
    let menu = gio::Menu::new();
    let targets = gio::Menu::new();
    let open_in = open_in(state);
    if open_in.n_items() == 0 {
        // An action nobody adds: GTK shows the entry insensitive.
        targets.append_item(&item("Open In", "no-targets", None));
    } else {
        targets.append_submenu(Some("Open In"), &open_in);
    }
    menu.append_section(None, &targets);
    let link = gio::Menu::new();
    link.append_item(&item("Copy Link", COPY_LINK, None));
    link.append_item(&item("Create Rule…", CREATE_RULE, None));
    menu.append_section(None, &link);
    let settings = gio::Menu::new();
    settings.append_item(&item("Settings…", SETTINGS, None));
    menu.append_section(None, &settings);
    menu
}

/// Open In (PICK-28): one section per group, the group's label as its
/// heading (TGT-02).
fn open_in(state: &PickerState) -> gio::Menu {
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    let mut heading: Option<String> = None;
    for row in model::open_in(state) {
        if row.kind == "header" {
            if section.n_items() > 0 {
                menu.append_section(heading.as_deref(), &section);
            }
            section = gio::Menu::new();
            heading = Some(row.label);
            continue;
        }
        let target = format!("{}:{}", row.group, row.item);
        section.append_item(&item(&row.label, OPEN_IN, Some(&target)));
    }
    if section.n_items() > 0 {
        menu.append_section(heading.as_deref(), &section);
    }
    menu
}

/// Open In's icons (PICK-28): each target's, with its profile badge.
pub fn open_in_icons(state: &PickerState) -> Vec<ItemIcon> {
    model::open_in(state)
        .into_iter()
        .filter(|row| row.kind != "header")
        .map(|row| {
            let badge = state
                .view
                .overflow
                .get(row.group)
                .and_then(|group| group.entries.get(row.item))
                .and_then(|entry| entry.badge.as_ref())
                .and_then(|badge| serde_json::to_value(badge).ok());
            ItemIcon {
                label: row.label,
                source: row.icon,
                badge,
            }
        })
        .collect()
}

/// Tile `index`'s menu (PICK-30): Open and the ways the target supports,
/// then Make Primary Browser.
pub fn tile(state: &PickerState, index: usize) -> gio::Menu {
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    for entry in state.tile_menu(index) {
        if entry.action.is_empty() {
            menu.append_section(None, &section);
            section = gio::Menu::new();
            continue;
        }
        let target = format!("{index}:{}", entry.action);
        section.append_item(&item(&entry.label, TILE, Some(&target)));
    }
    if section.n_items() > 0 {
        menu.append_section(None, &section);
    }
    menu
}

/// `"<a>:<b>"` read back from a menu target.
pub fn pair(target: &str) -> Option<(usize, &str)> {
    let (first, second) = target.split_once(':')?;
    Some((first.parse().ok()?, second))
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    fn labels(menu: &gio::Menu) -> Vec<String> {
        (0..menu.n_items())
            .flat_map(|index| {
                let own = menu
                    .item_attribute_value(index, "label", None)
                    .and_then(|value| value.get::<String>());
                let section = menu
                    .item_link(index, "section")
                    .and_then(|link| link.downcast::<gio::Menu>().ok())
                    .map(|section| labels(&section))
                    .unwrap_or_default();
                own.into_iter().chain(section)
            })
            .collect()
    }

    #[test]
    fn the_overflow_menu_has_open_in_then_link_actions_then_settings() {
        // PICK-08.
        let state = PickerState::new(fixture::view());
        assert_eq!(
            labels(&overflow(&state)),
            ["Open In", "Copy Link", "Create Rule…", "Settings…"]
        );
        let open_in = open_in(&state);
        assert_eq!(
            open_in
                .item_attribute_value(0, "label", None)
                .and_then(|value| value.get::<String>())
                .as_deref(),
            Some("Browsers"),
            "a group's label heads its section (TGT-02)"
        );
        assert_eq!(labels(&open_in), ["Browsers", "Brave"]);
        let icons = open_in_icons(&state);
        assert_eq!(icons.len(), 1, "one icon per target, none for headings");
        assert_eq!(icons[0].label, "Brave");
    }

    #[test]
    fn the_tile_menu_keeps_the_shared_entries_and_separators() {
        // PICK-30: Work (a Chrome profile) has no private windows.
        let state = PickerState::new(fixture::view());
        let menu = tile(&state, 1);
        let all = labels(&menu);
        assert!(all.contains(&"Make Primary Browser".to_owned()), "{all:?}");
        assert!(
            !all.iter().any(|label| label.contains("Private")),
            "{all:?}"
        );
        assert!(menu.n_items() >= 2, "the separator starts a section");
    }

    #[test]
    fn targets_read_back() {
        assert_eq!(pair("2:open-private"), Some((2, "open-private")));
        assert_eq!(pair("x:open"), None);
        assert_eq!(pair("3"), None);
    }
}
