//! The tray menu's rows ([`super::model::Row`]) as a `GMenu` for a
//! `GtkPopoverMenu`, with one action per choosable row (TRAY-08):
//!
//! - a header names the section that follows it; a separator starts a new
//!   section;
//! - an action is a plain item, a radio item (the primary browser) is an
//!   item whose action holds the checked row's ID, so GTK draws the dot;
//! - a submenu nests (TRAY-15: More, then Recent Links); one that cannot
//!   open (disabled or empty) stays as an insensitive item;
//! - the shortcut is shown on the right (TRAY-13);
//! - a row's icon (the browsers') is listed in [`Built::icons`], for
//!   [`crate::widgets::menu_icons::add`] once the popover has the model.
//!
//! Each action is `tray.row<N>`; [`Built::ids`] maps it back to the row.

use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::model::Row;
use crate::widgets::menu_icons::ItemIcon;

/// The action group of the popup window.
pub const GROUP: &str = "tray";

/// The model and what its actions stand for.
#[derive(Debug)]
pub struct Built {
    pub menu: gio::Menu,
    /// Action name (without `tray.`) to row ID.
    pub ids: HashMap<String, String>,
    /// The actions, enabled as their rows are.
    pub actions: Vec<gio::SimpleAction>,
    /// The rows' icons, in menu order.
    pub icons: Vec<ItemIcon>,
}

/// The menu for `rows`.
pub fn build(rows: &[Row]) -> Built {
    let mut built = Built {
        menu: gio::Menu::new(),
        ids: HashMap::new(),
        actions: Vec::new(),
        icons: Vec::new(),
    };
    let menu = level(rows, &mut built);
    built.menu = menu;
    built
}

fn level(rows: &[Row], built: &mut Built) -> gio::Menu {
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    let mut heading: Option<String> = None;
    let close = |menu: &gio::Menu, section: &gio::Menu, heading: Option<&str>| {
        if section.n_items() > 0 || heading.is_some() {
            menu.append_section(heading, section);
        }
    };
    for row in rows {
        match row.kind {
            "header" | "separator" => {
                close(&menu, &section, heading.as_deref());
                section = gio::Menu::new();
                heading = (row.kind == "header").then(|| row.label.clone());
            }
            "submenu" if row.opens => {
                let submenu = level(&row.children, built);
                let item = gio::MenuItem::new_submenu(Some(&row.label), &submenu);
                decorate(&item, row, built);
                section.append_item(&item);
            }
            _ => section.append_item(&item(row, built)),
        }
    }
    close(&menu, &section, heading.as_deref());
    menu
}

fn item(row: &Row, built: &mut Built) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(&row.label), None);
    decorate(&item, row, built);
    if !row.selectable {
        // An action nobody adds: GTK shows the entry insensitive.
        item.set_detailed_action(&format!("{GROUP}.unavailable"));
        return item;
    }
    let name = format!("row{}", built.actions.len());
    let action = if row.kind == "radio" {
        let checked = if row.checked { row.id.as_str() } else { "" };
        let action = gio::SimpleAction::new_stateful(
            &name,
            Some(glib::VariantTy::STRING),
            &checked.to_variant(),
        );
        item.set_action_and_target_value(
            Some(&format!("{GROUP}.{name}")),
            Some(&row.id.to_variant()),
        );
        action
    } else {
        item.set_detailed_action(&format!("{GROUP}.{name}"));
        gio::SimpleAction::new(&name, None)
    };
    action.set_enabled(row.enabled);
    built.ids.insert(name, row.id.clone());
    built.actions.push(action);
    item
}

/// The icon (drawn by [`crate::widgets::menu_icons`]) and the shortcut label.
fn decorate(item: &gio::MenuItem, row: &Row, built: &mut Built) {
    if !row.icon.is_empty() {
        built.icons.push(ItemIcon {
            label: row.label.clone(),
            source: row.icon.clone(),
            badge: None,
        });
    }
    if let Some(accel) = accel(&row.shortcut) {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
}

/// The tray's shortcut text (`P`, `1`, `Ctrl+,`) as a GTK accelerator
/// (`p`, `1`, `<Control>comma`), for the label only; `None` when there is
/// none or it names a modifier or punctuation GTK has no name for here.
pub fn accel(shortcut: &str) -> Option<String> {
    if shortcut.is_empty() {
        return None;
    }
    // The key is the last part; a lone `+` is the plus key.
    let (modifiers, key) = match shortcut.rsplit_once('+') {
        Some((head, "")) => (head.strip_suffix('+').unwrap_or(head), "+"),
        Some((head, key)) => (head, key),
        None => ("", shortcut),
    };
    let mut accel = String::new();
    for modifier in modifiers.split('+').filter(|part| !part.is_empty()) {
        accel.push_str(match modifier {
            "Ctrl" | "Control" => "<Control>",
            "Alt" => "<Alt>",
            "Shift" => "<Shift>",
            "Super" | "Meta" => "<Super>",
            _ => return None,
        });
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(single), None) if single.is_ascii_alphanumeric() => {
            accel.push(single.to_ascii_lowercase());
        }
        (Some(single), None) => accel.push_str(punctuation(single)?),
        _ => accel.push_str(key),
    }
    Some(accel)
}

/// The keysym name of a punctuation key a shortcut may name.
fn punctuation(key: char) -> Option<&'static str> {
    Some(match key {
        ',' => "comma",
        '.' => "period",
        '+' => "plus",
        '-' => "minus",
        '=' => "equal",
        '/' => "slash",
        ';' => "semicolon",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_become_accelerators() {
        assert_eq!(accel("P").as_deref(), Some("p"));
        assert_eq!(accel("1").as_deref(), Some("1"));
        assert_eq!(accel("Ctrl+,").as_deref(), Some("<Control>comma"));
        assert_eq!(accel("Ctrl++").as_deref(), Some("<Control>plus"));
        assert_eq!(accel(""), None);
        assert_eq!(accel("Hyper+x"), None);
        assert_eq!(accel("Ctrl+Q").as_deref(), Some("<Control>q"));
        assert_eq!(accel("Ctrl+\\"), None);
    }
}
