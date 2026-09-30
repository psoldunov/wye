//! The core tray model as the `Tray` property sends it (`wye_api::tray`).

use wye_api::tray::{self as dto, PICKER_ICON, TrayItemKind, TrayOverlay};
use wye_core::tray::{ItemIcon, ItemKind, TrayIconSpec, TrayItem, TrayMenu};

/// `menu` as the `Tray` property's JSON model.
#[must_use]
pub(crate) fn menu(menu: &TrayMenu) -> dto::TrayMenu {
    dto::TrayMenu {
        icon: icon(&menu.icon),
        overlay: menu.warning.then_some(TrayOverlay::Warning),
        visible: menu.visible,
        items: menu.items.iter().map(item).collect(),
    }
}

/// TRAY-02, GEN-02.
fn icon(icon: &TrayIconSpec) -> dto::TrayIcon {
    match icon {
        TrayIconSpec::App => dto::TrayIcon::App,
        TrayIconSpec::Picker => dto::TrayIcon::Picker,
        TrayIconSpec::Named(name) => dto::TrayIcon::Theme { name: name.clone() },
    }
}

fn item(item: &TrayItem) -> dto::TrayItem {
    dto::TrayItem {
        id: item.id.clone(),
        kind: kind(item.kind),
        label: item.label.clone(),
        // TRAY-14: the app's own icon; the picker glyph for the Picker.
        icon: item.icon.as_ref().map(|icon| match icon {
            ItemIcon::Picker => PICKER_ICON.to_owned(),
            ItemIcon::Named(name) => name.clone(),
        }),
        // TRAY-13, KEY-51: as the interface shows it, `P`, `1`, `Ctrl+,`.
        shortcut: item
            .shortcut
            .as_ref()
            .map(wye_core::keybinding::KeyBinding::label),
        enabled: item.enabled,
        checked: item.checked,
        children: item.children.iter().map(self::item).collect(),
    }
}

const fn kind(kind: ItemKind) -> TrayItemKind {
    match kind {
        ItemKind::Action => TrayItemKind::Action,
        ItemKind::Header => TrayItemKind::Header,
        ItemKind::Radio => TrayItemKind::Radio,
        ItemKind::Separator => TrayItemKind::Separator,
        ItemKind::Submenu => TrayItemKind::Submenu,
    }
}

#[cfg(test)]
mod tests {
    use wye_core::Config;
    use wye_core::config::TrayIcon as IconStyle;
    use wye_core::target_menu::TargetCatalog;
    use wye_core::tray::{RecentLink, TrayStatus, ids};

    use super::*;

    fn built(config: &Config, status: &TrayStatus) -> dto::TrayMenu {
        menu(&TrayMenu::build(config, &TargetCatalog::default(), status))
    }

    fn find<'a>(items: &'a [dto::TrayItem], id: &str) -> Option<&'a dto::TrayItem> {
        items.iter().find_map(|item| {
            if item.id == id {
                Some(item)
            } else {
                find(&item.children, id)
            }
        })
    }

    #[test]
    fn the_default_menu_follows_the_spec_layout() {
        let status = TrayStatus {
            wye_is_default: true,
            ..TrayStatus::default()
        };
        let menu = built(&Config::default(), &status);
        let kinds: Vec<_> = menu.items.iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            [
                TrayItemKind::Action,
                TrayItemKind::Separator,
                TrayItemKind::Header,
                TrayItemKind::Radio,
                TrayItemKind::Separator,
                TrayItemKind::Action,
                TrayItemKind::Submenu,
                TrayItemKind::Separator,
                TrayItemKind::Action,
            ]
        );
        assert_eq!(menu.overlay, None);
        assert!(menu.visible);
        let clipboard = find(&menu.items, ids::OPEN_CLIPBOARD).expect("clipboard item");
        assert!(!clipboard.enabled, "TRAY-10: no URL on the clipboard");
        let picker = find(&menu.items, ids::PRIMARY_PICKER).expect("picker item");
        assert!(picker.checked);
        assert_eq!(picker.icon.as_deref(), Some(PICKER_ICON));
        assert_eq!(picker.shortcut.as_deref(), Some("P"));
        let settings = find(&menu.items, ids::SETTINGS).expect("settings");
        assert_eq!(settings.shortcut.as_deref(), Some("Ctrl+,"));
        let quit = find(&menu.items, ids::QUIT).expect("quit");
        assert_eq!(quit.shortcut.as_deref(), Some("Ctrl+Q"));
        assert_eq!(
            menu.icon,
            dto::TrayIcon::Picker,
            "TRAY-02: the picker glyph"
        );
    }

    #[test]
    fn not_being_the_default_adds_the_item_and_the_overlay_tray_18() {
        let menu = built(&Config::default(), &TrayStatus::default());
        assert_eq!(menu.items[0].id, ids::MAKE_DEFAULT);
        assert_eq!(menu.items[1].kind, TrayItemKind::Separator);
        assert_eq!(menu.overlay, Some(TrayOverlay::Warning));
    }

    #[test]
    fn the_wye_style_and_the_hidden_icon_come_through_gen_02_gen_03() {
        let mut config = Config::default();
        config.general.tray_icon = IconStyle::Wye;
        config.general.show_tray_icon = false;
        let menu = built(&config, &TrayStatus::default());
        assert_eq!(menu.icon, dto::TrayIcon::App);
        assert!(!menu.visible);
    }

    #[test]
    fn recent_links_appear_while_history_is_on_tray_15() {
        let mut config = Config::default();
        config.advanced.history = true;
        let status = TrayStatus {
            clipboard_has_url: true,
            wye_is_default: true,
            recent: vec![RecentLink {
                id: "7".to_owned(),
                url: "https://example.com/a/b".to_owned(),
            }],
        };
        let menu = built(&config, &status);
        let recent = find(&menu.items, "recent:7").expect("the recent link");
        assert_eq!(recent.kind, TrayItemKind::Action);
        assert!(recent.label.contains("example.com"));
        assert!(
            find(&menu.items, ids::OPEN_CLIPBOARD)
                .expect("clipboard")
                .enabled
        );
    }
}
