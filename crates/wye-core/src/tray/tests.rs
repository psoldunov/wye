use super::*;
use crate::config::{ShownEntry, TrayIcon};
use crate::target::DesktopId;
use crate::target_menu::{HandlerEntry, ProfileEntry};

fn id(name: &str) -> DesktopId {
    DesktopId::new(name).unwrap()
}

fn app(name: &str) -> Target {
    Target::App(id(name))
}

fn handler(app: &str, name: &str, profiles: &[(&str, &str)]) -> HandlerEntry {
    HandlerEntry {
        app: id(app),
        name: name.to_owned(),
        icon: Some(format!("{}-icon", name.to_lowercase())),
        browser: true,
        private: true,
        new_window: true,
        profiles: profiles
            .iter()
            .map(|(id, name)| ProfileEntry {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                badge: None,
            })
            .collect(),
    }
}

fn catalog() -> TargetCatalog {
    TargetCatalog {
        handlers: vec![
            handler("firefox.desktop", "Firefox", &[]),
            handler("google-chrome.desktop", "Chrome", &[("Profile 1", "Work")]),
        ],
        ..TargetCatalog::default()
    }
}

fn work() -> Target {
    Target::Profile {
        app: id("google-chrome.desktop"),
        id: "Profile 1".into(),
    }
}

/// The example of 01-tray-menu.md: one browser and one browser profile.
fn config() -> Config {
    let mut config = Config::default();
    config.browsers.shown = vec![
        ShownEntry {
            target: app("firefox.desktop"),
            hotkey: None,
        },
        ShownEntry {
            target: work(),
            hotkey: None,
        },
    ];
    config
}

fn status() -> TrayStatus {
    TrayStatus {
        wye_is_default: true,
        ..TrayStatus::default()
    }
}

fn build(config: &Config, status: &TrayStatus) -> TrayMenu {
    TrayMenu::build(config, &catalog(), status)
}

/// A compact rendering: kind and label of each item.
fn outline(items: &[TrayItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item.kind {
            ItemKind::Separator => "---".to_owned(),
            ItemKind::Header => format!("# {}", item.label),
            ItemKind::Radio => format!(
                "{} {}",
                if item.checked { "(•)" } else { "( )" },
                item.label
            ),
            ItemKind::Submenu => format!("{} >", item.label),
            ItemKind::Action => item.label.clone(),
        })
        .collect()
}

fn shortcut(menu: &TrayMenu, id: &str) -> Option<String> {
    menu.find(id)?.shortcut.as_ref().map(KeyBinding::label)
}

// 01-tray-menu.md, "Menu layout": items in exact order.
#[test]
fn layout_matches_the_spec_example() {
    let menu = build(&config(), &status());
    assert_eq!(
        outline(&menu.items),
        [
            "Open URL from Clipboard",
            "---",
            "# Primary Browser",
            "(•) Picker",
            "( ) Firefox",
            "( ) Work",
            "---",
            "Settings…",
            "More >",
            "---",
            "Quit Wye",
        ]
    );
}

// TRAY-13, KEY-50, KEY-51
#[test]
fn shortcuts_are_p_digits_and_the_fixed_keys() {
    let menu = build(&config(), &status());
    assert_eq!(shortcut(&menu, ids::PRIMARY_PICKER).as_deref(), Some("P"));
    assert_eq!(shortcut(&menu, &ids::primary(0)).as_deref(), Some("1"));
    assert_eq!(shortcut(&menu, &ids::primary(1)).as_deref(), Some("2"));
    assert_eq!(shortcut(&menu, ids::SETTINGS).as_deref(), Some("Ctrl+,"));
    assert_eq!(shortcut(&menu, ids::QUIT).as_deref(), Some("Ctrl+Q"));
    assert_eq!(shortcut(&menu, ids::OPEN_CLIPBOARD), None);
    assert_eq!(shortcut(&menu, ids::MORE), None);
}

// TRAY-13: items beyond the ninth get no digit.
#[test]
fn only_the_first_nine_browsers_get_a_digit() {
    let mut config = Config::default();
    let mut catalog = TargetCatalog::default();
    for i in 0..11 {
        let name = format!("b{i}.desktop");
        catalog
            .handlers
            .push(handler(&name, &format!("B{i:02}"), &[]));
        config.browsers.shown.push(ShownEntry {
            target: app(&name),
            hotkey: None,
        });
    }
    let menu = TrayMenu::build(&config, &catalog, &status());
    for position in 0..9 {
        assert_eq!(
            shortcut(&menu, &ids::primary(position)),
            Some((position + 1).to_string())
        );
    }
    assert_eq!(shortcut(&menu, &ids::primary(9)), None);
    assert_eq!(shortcut(&menu, &ids::primary(10)), None);
    assert!(menu.find(&ids::primary(10)).is_some(), "still listed");
}

// TRAY-10
#[test]
fn the_clipboard_item_follows_the_clipboard() {
    let menu = build(&config(), &status());
    assert!(!menu.find(ids::OPEN_CLIPBOARD).unwrap().enabled);
    let with_url = TrayStatus {
        clipboard_has_url: true,
        ..status()
    };
    assert!(
        build(&config(), &with_url)
            .find(ids::OPEN_CLIPBOARD)
            .unwrap()
            .enabled
    );
}

// TRAY-11
#[test]
fn the_primary_browser_is_the_checked_radio_item() {
    let checked = |primary: Target| {
        let mut config = config();
        config.browsers.primary = primary;
        let menu = build(&config, &status());
        menu.items
            .iter()
            .filter(|i| i.kind == ItemKind::Radio && i.checked)
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(checked(Target::Picker), [ids::PRIMARY_PICKER]);
    assert_eq!(checked(app("firefox.desktop")), [ids::primary(0)]);
    assert_eq!(checked(work()), [ids::primary(1)]);
    // A primary browser that is not shown leaves no radio item checked.
    assert!(checked(app("elsewhere.desktop")).is_empty());
}

// TRAY-11: each radio item carries what choosing it sets.
#[test]
fn radio_items_carry_their_target() {
    let menu = build(&config(), &status());
    assert_eq!(
        menu.find(ids::PRIMARY_PICKER).unwrap().target,
        Some(Target::Picker)
    );
    assert_eq!(
        menu.find(&ids::primary(0)).unwrap().target,
        Some(app("firefox.desktop"))
    );
    assert_eq!(menu.find(&ids::primary(1)).unwrap().target, Some(work()));
    assert_eq!(menu.find(ids::SETTINGS).unwrap().target, None);
}

// TRAY-12
#[test]
fn a_profile_shows_only_its_name_with_the_browsers_icon() {
    let menu = build(&config(), &status());
    let item = menu.find(&ids::primary(1)).unwrap();
    assert_eq!(item.label, "Work");
    assert_eq!(item.icon, Some(ItemIcon::Named("chrome-icon".into())));
    assert_eq!(
        menu.find(ids::PRIMARY_PICKER).unwrap().icon,
        Some(ItemIcon::Picker)
    );
}

#[test]
fn the_section_header_is_dimmed_and_not_interactive() {
    let menu = build(&config(), &status());
    let header = menu.find(ids::PRIMARY_HEADER).unwrap();
    assert_eq!(header.kind, ItemKind::Header);
    assert!(!header.enabled);
}

// TRAY-02
#[test]
fn the_icon_follows_the_setting_and_the_primary_browser() {
    let icon = |tray: TrayIcon, primary: Target| {
        let mut config = config();
        config.general.tray_icon = tray;
        config.browsers.primary = primary;
        build(&config, &status()).icon
    };
    assert_eq!(
        icon(TrayIcon::PrimaryBrowser, Target::Picker),
        TrayIconSpec::Picker
    );
    assert_eq!(
        icon(TrayIcon::PrimaryBrowser, app("firefox.desktop")),
        TrayIconSpec::Named("firefox-icon".into())
    );
    assert_eq!(
        icon(TrayIcon::PrimaryBrowser, work()),
        TrayIconSpec::Named("chrome-icon".into())
    );
    assert_eq!(
        icon(TrayIcon::Wye, app("firefox.desktop")),
        TrayIconSpec::App
    );
    assert_eq!(icon(TrayIcon::Wye, Target::Picker), TrayIconSpec::App);
    // A primary browser that is gone falls back to Wye's own icon.
    assert_eq!(
        icon(TrayIcon::PrimaryBrowser, app("gone.desktop")),
        TrayIconSpec::App
    );
}

// TRAY-03: the icon changes as soon as the primary browser does.
#[test]
fn changing_the_primary_browser_changes_the_menu() {
    let mut config = config();
    let before = build(&config, &status());
    config.browsers.primary = app("firefox.desktop");
    assert_ne!(build(&config, &status()), before);
}

// TRAY-04
#[test]
fn the_icon_hides_with_show_tray_icon_off() {
    let mut config = config();
    assert!(build(&config, &status()).visible);
    config.general.show_tray_icon = false;
    assert!(!build(&config, &status()).visible);
}

// TRAY-18
#[test]
fn not_being_default_adds_the_takeover_item_and_a_warning() {
    let not_default = TrayStatus {
        wye_is_default: false,
        ..status()
    };
    let menu = build(&config(), &not_default);
    assert_eq!(
        outline(&menu.items)[..3],
        ["Make Wye Default Browser", "---", "Open URL from Clipboard"]
    );
    assert!(menu.warning);
    assert!(menu.find(ids::MAKE_DEFAULT).unwrap().enabled);

    let default = build(&config(), &status());
    assert!(!default.warning);
    assert!(default.find(ids::MAKE_DEFAULT).is_none());
}

// TRAY-15
#[test]
fn the_more_submenu_lists_its_items_in_order() {
    let menu = build(&config(), &status());
    let more = menu.find(ids::MORE).unwrap();
    assert_eq!(more.kind, ItemKind::Submenu);
    assert_eq!(
        outline(&more.children),
        [
            "History…",
            "---",
            "Test Rules…",
            "Rescan Browsers",
            "Set Up Wye…",
            "---",
            "Help",
            "About Wye",
        ],
        "Recent Links is absent while history is off"
    );
}

// TRAY-15
#[test]
fn recent_links_appear_only_while_history_is_on() {
    let mut config = config();
    config.advanced.history = true;
    let recent: Vec<RecentLink> = (0..12)
        .map(|i| RecentLink {
            id: i.to_string(),
            url: format!("https://www.example.com/page/{i}"),
        })
        .collect();
    let status = TrayStatus { recent, ..status() };
    let menu = build(&config, &status);
    let more = menu.find(ids::MORE).unwrap();
    assert_eq!(more.children[0].label, "History…");
    let recent = &more.children[1];
    assert_eq!(recent.id, ids::RECENT);
    assert_eq!(recent.kind, ItemKind::Submenu);
    assert!(recent.enabled);
    assert_eq!(recent.children.len(), RECENT_LIMIT, "the last 10");
    assert_eq!(recent.children[0].label, "example.com/page/0");
    assert_eq!(recent.children[0].id, "recent:0");
    assert_eq!(menu.find("recent:9").unwrap().label, "example.com/page/9");
    assert!(menu.find("recent:10").is_none());

    config.advanced.history = false;
    assert!(
        build(&config, &status_with_recent())
            .find(ids::RECENT)
            .is_none()
    );
}

fn collect_ids(items: &[TrayItem], seen: &mut Vec<String>) {
    for item in items {
        seen.push(item.id.clone());
        collect_ids(&item.children, seen);
    }
}

fn status_with_recent() -> TrayStatus {
    TrayStatus {
        recent: vec![RecentLink {
            id: "1".into(),
            url: "https://example.com/".into(),
        }],
        ..status()
    }
}

#[test]
fn recent_links_are_middle_truncated_and_an_empty_list_is_disabled() {
    let mut config = config();
    config.advanced.history = true;
    let long = RecentLink {
        id: "a".into(),
        url: format!("https://example.com/{}/end", "segment/".repeat(20)),
    };
    let status = TrayStatus {
        recent: vec![long],
        ..status()
    };
    let label = build(&config, &status)
        .find("recent:a")
        .unwrap()
        .label
        .clone();
    assert_eq!(label.chars().count(), RECENT_LABEL_CHARS);
    assert!(label.starts_with("example.com/"));
    assert!(label.ends_with("/end"));
    assert!(label.contains('…'));

    let empty = build(&config, &self::status());
    let recent = empty.find(ids::RECENT).unwrap();
    assert!(!recent.enabled);
    assert!(recent.children.is_empty());
}

#[test]
fn every_item_id_is_unique() {
    let mut config = config();
    config.advanced.history = true;
    let menu = build(&config, &status_with_recent());
    let mut seen = Vec::new();
    collect_ids(&menu.items, &mut seen);
    let mut sorted = seen.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len(), "{seen:?}");
    assert!(seen.contains(&"separator:0".to_owned()));
}

#[test]
fn an_unconfigured_menu_lists_the_installed_browsers() {
    let menu = build(&Config::default(), &status());
    let radios: Vec<_> = menu
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Radio)
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(radios, ["Picker", "Chrome", "Firefox"]);
}

#[test]
fn the_menu_serialises_for_the_api() {
    let menu = build(&config(), &status());
    let json = serde_json::to_value(&menu).unwrap();
    assert_eq!(json["icon"], "picker");
    assert_eq!(json["visible"], true);
    assert_eq!(json["items"][0]["id"], "open-clipboard");
    assert_eq!(json["items"][0]["kind"], "action");
    assert_eq!(json["items"][3]["shortcut"], "p");
    assert_eq!(
        json["items"][3]["target"],
        serde_json::json!({ "picker": true })
    );
}

fn held(modifiers: &[Modifier]) -> Modifiers {
    Modifiers::from_slice(modifiers)
}

#[test]
fn a_plain_click_sets_the_primary_browser_tray_11() {
    let menu = build(&config(), &status());
    assert_eq!(
        menu.primary_choice(&ids::primary(1), Modifiers::NONE),
        Some(PrimaryChoice::SetPrimary(work()))
    );
    assert_eq!(
        menu.primary_choice(ids::PRIMARY_PICKER, Modifiers::NONE),
        Some(PrimaryChoice::SetPrimary(Target::Picker))
    );
    // Only Ctrl and Shift open (TRAY-20).
    assert_eq!(
        menu.primary_choice(&ids::primary(0), held(&[Modifier::Alt, Modifier::Super])),
        Some(PrimaryChoice::SetPrimary(app("firefox.desktop")))
    );
}

#[test]
fn ctrl_or_shift_opens_the_target_instead_tray_20() {
    let menu = build(&config(), &status());
    for modifiers in [
        held(&[Modifier::Ctrl]),
        held(&[Modifier::Shift]),
        held(&[Modifier::Ctrl, Modifier::Shift, Modifier::Alt]),
    ] {
        assert_eq!(
            menu.primary_choice(&ids::primary(0), modifiers),
            Some(PrimaryChoice::Open(app("firefox.desktop"))),
            "{modifiers}"
        );
        assert_eq!(
            menu.primary_choice(&ids::primary(1), modifiers),
            Some(PrimaryChoice::Open(work())),
            "{modifiers}"
        );
    }
}

#[test]
fn the_picker_has_nothing_to_open_tray_20() {
    let menu = build(&config(), &status());
    assert_eq!(
        menu.primary_choice(ids::PRIMARY_PICKER, held(&[Modifier::Ctrl])),
        Some(PrimaryChoice::Nothing)
    );
}

#[test]
fn only_primary_browser_items_have_a_choice() {
    let menu = build(&config(), &status());
    for id in [ids::SETTINGS, ids::PRIMARY_HEADER, "primary:9", "nothing"] {
        assert_eq!(
            menu.primary_choice(id, held(&[Modifier::Ctrl])),
            None,
            "{id}"
        );
    }
}
