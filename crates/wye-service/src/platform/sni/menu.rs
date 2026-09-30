//! The `Tray` model as a `DBusMenu` (01-tray-menu.md "Linux notes"):
//! separators, disabled headers, one radio group for the primary browser
//! (TRAY-11), submenus (TRAY-15), icons (TRAY-14) and shortcuts (TRAY-13).

use ksni::MenuItem;
use ksni::menu::{RadioGroup, RadioItem, StandardItem, SubMenu};
use wye_api::tray::{TrayItem, TrayItemKind};

/// What a menu item does when chosen: tell the tray which ID it was.
pub trait Chooser: Sized + Send + 'static {
    /// The item `id` was chosen.
    fn chosen(&mut self, id: &str);
}

/// The `DBusMenu` items for `items`. Consecutive radio items form one radio
/// group, since `DBusMenu` groups radios by position.
#[must_use]
pub fn items<T: Chooser>(items: &[TrayItem]) -> Vec<MenuItem<T>> {
    let mut built = Vec::new();
    let mut radios: Vec<&TrayItem> = Vec::new();
    for item in items {
        if item.kind == TrayItemKind::Radio {
            radios.push(item);
            continue;
        }
        if !radios.is_empty() {
            built.push(radio_group(&std::mem::take(&mut radios)));
        }
        built.push(item_for(item));
    }
    if !radios.is_empty() {
        built.push(radio_group(&radios));
    }
    built
}

fn item_for<T: Chooser>(item: &TrayItem) -> MenuItem<T> {
    match item.kind {
        TrayItemKind::Separator => MenuItem::Separator,
        TrayItemKind::Submenu => SubMenu {
            label: label(&item.label),
            enabled: item.enabled,
            icon_name: item.icon.clone().unwrap_or_default(),
            submenu: items(&item.children),
            ..SubMenu::default()
        }
        .into(),
        TrayItemKind::Header | TrayItemKind::Action | TrayItemKind::Radio => {
            let id = item.id.clone();
            StandardItem {
                label: label(&item.label),
                // A header is never chosen (01-tray-menu.md: "Non-interactive, dimmed").
                enabled: item.enabled && item.kind != TrayItemKind::Header,
                icon_name: item.icon.clone().unwrap_or_default(),
                shortcut: shortcut(item.shortcut.as_deref()),
                activate: Box::new(move |tray: &mut T| tray.chosen(&id)),
                ..StandardItem::default()
            }
            .into()
        }
    }
}

fn radio_group<T: Chooser>(radios: &[&TrayItem]) -> MenuItem<T> {
    let ids: Vec<String> = radios.iter().map(|item| item.id.clone()).collect();
    RadioGroup {
        selected: radios.iter().position(|item| item.checked).unwrap_or(0),
        select: Box::new(move |tray: &mut T, index: usize| {
            if let Some(id) = ids.get(index) {
                tray.chosen(id);
            }
        }),
        options: radios
            .iter()
            .map(|item| RadioItem {
                label: label(&item.label),
                enabled: item.enabled,
                icon_name: item.icon.clone().unwrap_or_default(),
                shortcut: shortcut(item.shortcut.as_deref()),
                ..RadioItem::default()
            })
            .collect(),
    }
    .into()
}

/// A lone underscore would be read as a mnemonic by the menu host.
fn label(text: &str) -> String {
    text.replace('_', "__")
}

/// `Ctrl+,` as `DBusMenu` wants it: `[["Control", ","]]`.
fn shortcut(text: Option<&str>) -> Vec<Vec<String>> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Vec::new();
    };
    let keys = split_keys(text)
        .into_iter()
        .map(|part| match part {
            "Ctrl" => "Control".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    vec![keys]
}

/// `Ctrl+Shift++` is Ctrl, Shift and `+`: a trailing `+` is the key.
fn split_keys(text: &str) -> Vec<&str> {
    match text.strip_suffix("++") {
        Some(modifiers) => modifiers.split('+').chain(["+"]).collect(),
        None if text == "+" => vec!["+"],
        None => text.split('+').collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        chosen: Vec<String>,
    }

    impl Chooser for Recorder {
        fn chosen(&mut self, id: &str) {
            self.chosen.push(id.to_owned());
        }
    }

    fn item(id: &str, kind: TrayItemKind) -> TrayItem {
        TrayItem {
            id: id.to_owned(),
            kind,
            label: id.to_owned(),
            icon: None,
            shortcut: None,
            enabled: true,
            checked: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn shortcuts_use_dbusmenu_key_names_tray_13() {
        assert_eq!(shortcut(Some("Ctrl+,")), vec![vec!["Control", ","]]);
        assert_eq!(shortcut(Some("Ctrl+Q")), vec![vec!["Control", "Q"]]);
        assert_eq!(shortcut(Some("P")), vec![vec!["P"]]);
        assert_eq!(
            shortcut(Some("Ctrl+Shift++")),
            vec![vec!["Control", "Shift", "+"]]
        );
        assert!(shortcut(None).is_empty());
    }

    #[test]
    fn consecutive_radios_form_one_group_tray_11() {
        let first = item("primary:picker", TrayItemKind::Radio);
        let second = TrayItem {
            checked: true,
            ..item("primary:0", TrayItemKind::Radio)
        };
        let menu = [
            item("primary-header", TrayItemKind::Header),
            first,
            second,
            item("separator:0", TrayItemKind::Separator),
            item("quit", TrayItemKind::Action),
        ];
        let built = items::<Recorder>(&menu);
        assert_eq!(built.len(), 4, "header, one radio group, separator, quit");
        let MenuItem::RadioGroup(group) = &built[1] else {
            panic!("the radios are one group");
        };
        assert_eq!(group.selected, 1);
        assert_eq!(group.options.len(), 2);
        let mut recorder = Recorder::default();
        (group.select)(&mut recorder, 0);
        assert_eq!(recorder.chosen, vec!["primary:picker"]);
    }

    #[test]
    fn headers_are_disabled_and_actions_report_their_id() {
        let built = items::<Recorder>(&[
            item("primary-header", TrayItemKind::Header),
            item("settings", TrayItemKind::Action),
        ]);
        let MenuItem::Standard(header) = &built[0] else {
            panic!("a header is a standard item");
        };
        assert!(!header.enabled);
        let MenuItem::Standard(settings) = &built[1] else {
            panic!("an action is a standard item");
        };
        let mut recorder = Recorder::default();
        (settings.activate)(&mut recorder);
        assert_eq!(recorder.chosen, vec!["settings"]);
    }

    #[test]
    fn submenus_nest_and_labels_escape_mnemonics() {
        let more = TrayItem {
            children: vec![item("about", TrayItemKind::Action)],
            ..item("more", TrayItemKind::Submenu)
        };
        let named = TrayItem {
            label: "my_profile".to_owned(),
            ..item("primary:0", TrayItemKind::Action)
        };
        let built = items::<Recorder>(&[more, named]);
        let MenuItem::SubMenu(submenu) = &built[0] else {
            panic!("a submenu");
        };
        assert_eq!(submenu.submenu.len(), 1);
        let MenuItem::Standard(named) = &built[1] else {
            panic!("a standard item");
        };
        assert_eq!(named.label, "my__profile");
    }
}
