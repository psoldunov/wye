//! The `Tray` model as a `DBusMenu` (01-tray-menu.md "Linux notes"):
//! separators, disabled headers, one radio group for the primary browser
//! (TRAY-11), submenus (TRAY-15), icons (TRAY-14) and shortcuts (TRAY-13).

use std::path::Path;

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
///
/// `clicked` is the ID of the item the user last chose, until the menu is
/// published again. A `DBusMenu` host checks a clicked radio item by itself
/// (Plasma does), so the tray shows that item selected too: ksni then sees
/// what the host shows, and publishing the real menu afterwards makes it send
/// what is really checked (TRAY-11, TRAY-20: a Shift- or Ctrl-click opens the
/// browser and leaves the primary unchanged).
#[must_use]
pub fn items<T: Chooser>(items: &[TrayItem], clicked: Option<&str>) -> Vec<MenuItem<T>> {
    let mut built = Vec::new();
    let mut radios: Vec<&TrayItem> = Vec::new();
    for item in items {
        if item.kind == TrayItemKind::Radio {
            radios.push(item);
            continue;
        }
        if !radios.is_empty() {
            built.push(radio_group(&std::mem::take(&mut radios), clicked));
        }
        built.push(item_for(item, clicked));
    }
    if !radios.is_empty() {
        built.push(radio_group(&radios, clicked));
    }
    built
}

fn item_for<T: Chooser>(item: &TrayItem, clicked: Option<&str>) -> MenuItem<T> {
    match item.kind {
        TrayItemKind::Separator => MenuItem::Separator,
        TrayItemKind::Submenu => {
            let (icon_name, icon_data) = icon(item.icon.as_deref());
            SubMenu {
                label: label(&item.label),
                enabled: item.enabled,
                icon_name,
                icon_data,
                submenu: items(&item.children, clicked),
                ..SubMenu::default()
            }
            .into()
        }
        TrayItemKind::Header | TrayItemKind::Action | TrayItemKind::Radio => {
            let id = item.id.clone();
            let (icon_name, icon_data) = icon(item.icon.as_deref());
            StandardItem {
                label: label(&item.label),
                // A header is never chosen (01-tray-menu.md: "Non-interactive, dimmed").
                enabled: item.enabled && item.kind != TrayItemKind::Header,
                icon_name,
                icon_data,
                shortcut: shortcut(item.shortcut.as_deref()),
                activate: Box::new(move |tray: &mut T| tray.chosen(&id)),
                ..StandardItem::default()
            }
            .into()
        }
    }
}

fn radio_group<T: Chooser>(radios: &[&TrayItem], clicked: Option<&str>) -> MenuItem<T> {
    let ids: Vec<String> = radios.iter().map(|item| item.id.clone()).collect();
    // The clicked item, when it is in this group, else the checked one.
    let selected = clicked
        .and_then(|id| radios.iter().position(|item| item.id == id))
        .or_else(|| radios.iter().position(|item| item.checked))
        .unwrap_or(0);
    RadioGroup {
        selected,
        select: Box::new(move |tray: &mut T, index: usize| {
            if let Some(id) = ids.get(index) {
                tray.chosen(id);
            }
        }),
        options: radios
            .iter()
            .map(|item| {
                let (icon_name, icon_data) = icon(item.icon.as_deref());
                RadioItem {
                    label: label(&item.label),
                    enabled: item.enabled,
                    icon_name,
                    icon_data,
                    shortcut: shortcut(item.shortcut.as_deref()),
                    ..RadioItem::default()
                }
            })
            .collect(),
    }
    .into()
}

/// An item's `icon-name` and `icon-data` (TRAY-14). A theme name goes as
/// `icon-name`. A desktop entry may name its icon by absolute path, which
/// most menu hosts do not look up, so a PNG file goes as `icon-data`, the
/// PNG bytes `DBusMenu` takes; any other path stays a name.
fn icon(name: Option<&str>) -> (String, Vec<u8>) {
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        return (String::new(), Vec::new());
    };
    let path = Path::new(name);
    let png = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if path.is_absolute() && png {
        match std::fs::read(path) {
            Ok(bytes) => return (String::new(), bytes),
            Err(error) => tracing::debug!(%error, name, "cannot read the menu icon"),
        }
    }
    (name.to_owned(), Vec::new())
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
        let built = items::<Recorder>(&menu, None);
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

    fn selected(menu: &[TrayItem], clicked: Option<&str>) -> usize {
        let built = items::<Recorder>(menu, clicked);
        let Some(MenuItem::RadioGroup(group)) = built.first() else {
            panic!("the radios are one group");
        };
        group.selected
    }

    #[test]
    fn a_clicked_radio_is_selected_until_published_again_tray_20() {
        let menu = [
            item("primary:picker", TrayItemKind::Radio),
            TrayItem {
                checked: true,
                ..item("primary:0", TrayItemKind::Radio)
            },
            item("primary:1", TrayItemKind::Radio),
        ];
        assert_eq!(selected(&menu, None), 1, "the checked item");
        assert_eq!(selected(&menu, Some("primary:1")), 2, "the clicked item");
        assert_eq!(selected(&menu, Some("quit")), 1, "not in the group");
    }

    #[test]
    fn headers_are_disabled_and_actions_report_their_id() {
        let built = items::<Recorder>(
            &[
                item("primary-header", TrayItemKind::Header),
                item("settings", TrayItemKind::Action),
            ],
            None,
        );
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
    fn icons_by_name_stay_names_and_png_files_go_as_data_tray_14() {
        assert_eq!(icon(None), (String::new(), Vec::new()));
        assert_eq!(icon(Some("firefox")), ("firefox".to_owned(), Vec::new()));
        let dir = tempfile::tempdir().expect("temp dir");
        let png = dir.path().join("app.png");
        std::fs::write(&png, b"\x89PNG").expect("icon");
        let png = png.to_str().expect("UTF-8");
        assert_eq!(icon(Some(png)), (String::new(), b"\x89PNG".to_vec()));
        let svg = dir.path().join("app.svg");
        let svg = svg.to_str().expect("UTF-8");
        assert_eq!(icon(Some(svg)), (svg.to_owned(), Vec::new()));
        let missing = dir.path().join("gone.png");
        let missing = missing.to_str().expect("UTF-8");
        assert_eq!(icon(Some(missing)), (missing.to_owned(), Vec::new()));
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
        let built = items::<Recorder>(&[more, named], None);
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
