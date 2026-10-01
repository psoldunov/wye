use super::*;
use crate::config::{HotkeyScheme, PickerKeys, ShownEntry};
use crate::keybinding::KeyEvent;
use crate::keys::{Modifier, Modifiers};
use crate::pipeline::OpenOptions;
use crate::target::{CustomApp, DesktopId};
use crate::target_menu::{CustomEntry, HandlerEntry, ProfileEntry, TargetCaps};

fn id(name: &str) -> DesktopId {
    DesktopId::new(name).unwrap()
}

fn app(name: &str) -> Target {
    Target::App(id(name))
}

fn handler(app: &str, name: &str, private: bool, profiles: &[(&str, &str)]) -> HandlerEntry {
    HandlerEntry {
        app: id(app),
        name: name.to_owned(),
        icon: Some(name.to_lowercase()),
        browser: true,
        private,
        new_window: true,
        profiles: profiles
            .iter()
            .map(|(id, name)| ProfileEntry {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                badge: Some(Badge::Initial {
                    text: name.chars().take(1).collect(),
                    color: 0x33_99_ff,
                }),
            })
            .collect(),
    }
}

fn catalog() -> TargetCatalog {
    TargetCatalog {
        handlers: vec![
            handler("firefox.desktop", "Firefox", true, &[]),
            handler(
                "google-chrome.desktop",
                "Google Chrome",
                true,
                &[("Profile 1", "Work")],
            ),
            handler("app.zen_browser.zen.desktop", "Zen", false, &[]),
            handler("yandex.desktop", "Яндекс Браузер", false, &[]),
            handler("brave.desktop", "Brave", true, &[]),
        ],
        apps: Vec::new(),
        custom: vec![CustomEntry {
            app: CustomApp::Executable("tool".into()),
            name: "Tool".into(),
            icon: None,
        }],
    }
}

fn shown(targets: &[(Target, Option<&str>)]) -> Config {
    let mut config = Config::default();
    config.browsers.shown = targets
        .iter()
        .map(|(target, hotkey)| ShownEntry {
            target: target.clone(),
            hotkey: hotkey.map(str::to_owned),
        })
        .collect();
    config
}

fn work() -> Target {
    Target::Profile {
        app: id("google-chrome.desktop"),
        id: "Profile 1".into(),
    }
}

fn model(config: &Config) -> PickerModel {
    PickerModel::new(config, &catalog(), None, None)
}

fn mods(list: &[Modifier]) -> Modifiers {
    Modifiers::from_slice(list)
}

fn keys(model: &PickerModel) -> Vec<Option<String>> {
    model
        .tiles
        .iter()
        .map(|t| t.hotkey.as_ref().map(|h| h.key.clone()))
        .collect()
}

fn press(key: &str, modifiers: &[Modifier]) -> KeyEvent {
    KeyEvent::named(key, mods(modifiers))
}

// PICK-03, PICK-04, PICK-24
#[test]
fn tiles_follow_the_shown_order_and_the_first_is_selected() {
    let config = shown(&[
        (app("google-chrome.desktop"), Some("c")),
        (work(), Some("w")),
        (app("firefox.desktop"), None),
    ]);
    let model = model(&config);
    let names: Vec<_> = model.tiles.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Google Chrome", "Work (Google Chrome)", "Firefox"]);
    assert_eq!(model.selected, 0);
    assert_eq!(keys(&model), [Some("c".into()), Some("w".into()), None]);
    assert_eq!(model.tiles[0].hotkey.as_ref().unwrap().label, "C");
}

// PICK-06, PKS-04
#[test]
fn the_profile_badge_follows_the_setting() {
    let mut config = shown(&[(work(), None)]);
    assert!(model(&config).tiles[0].badge.is_some());
    config.picker.show_profile_badge = false;
    assert!(model(&config).tiles[0].badge.is_none());
}

// PICK-10, PICK-11, PKS-01, PKS-02
#[test]
fn icon_size_and_names_come_from_the_settings() {
    let mut config = shown(&[(app("firefox.desktop"), None)]);
    config.picker.icon_size = IconSize::Small;
    config.picker.show_names = false;
    let model = model(&config);
    assert_eq!(
        model.metrics,
        TileMetrics {
            icon: 24,
            pitch: 36,
            badge: 14
        }
    );
    assert!(!model.show_names);
    assert_eq!(TileMetrics::for_size(IconSize::Medium).icon, 32);
    assert_eq!(
        TileMetrics::for_size(IconSize::Large),
        TileMetrics {
            icon: 40,
            pitch: 60,
            badge: 24
        }
    );
}

// PICK-13
#[test]
fn rows_wrap_after_eight_tiles() {
    let spans = |count| -> Vec<(usize, usize)> {
        rows(count).into_iter().map(|r| (r.start, r.end)).collect()
    };
    assert!(spans(0).is_empty());
    assert_eq!(spans(1), [(0, 1)]);
    assert_eq!(spans(8), [(0, 8)]);
    assert_eq!(spans(9), [(0, 8), (8, 9)]);
    assert_eq!(spans(17), [(0, 8), (8, 16), (16, 17)]);
}

// PICK-03: with nothing chosen the installed browsers are shown.
#[test]
fn an_unconfigured_picker_shows_the_installed_browsers() {
    let model = model(&Config::default());
    let names: Vec<_> = model.tiles.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        ["Brave", "Firefox", "Google Chrome", "Zen", "Яндекс Браузер"]
    );
    assert_eq!(model.rows.len(), 1);
    assert_eq!(model.rows[0], 0..5);
    assert!(!model.is_empty());
}

// PICK-08, PICK-28
#[test]
fn the_overflow_menu_lists_what_is_not_a_tile() {
    let config = shown(&[(app("firefox.desktop"), None), (work(), None)]);
    let model = model(&config);
    let [
        OverflowEntry::OpenIn(menu),
        OverflowEntry::Separator,
        OverflowEntry::Action(OverflowAction::CopyLink),
        OverflowEntry::Action(OverflowAction::CreateRule),
        OverflowEntry::Separator,
        OverflowEntry::Action(OverflowAction::Settings),
    ] = model.overflow.as_slice()
    else {
        panic!("unexpected overflow layout: {:?}", model.overflow);
    };
    assert!(menu.find(&app("firefox.desktop")).is_none());
    assert!(menu.find(&work()).is_none());
    assert!(menu.find(&app("google-chrome.desktop")).is_some());
    assert!(menu.find(&Target::Private(id("firefox.desktop"))).is_some());
    assert!(menu.find(&Target::Picker).is_none());
    assert!(menu.find(&Target::Default).is_none());
    assert!(menu.items().any(|i| i.label == "Other…"));
    assert_eq!(OverflowAction::CreateRule.label(), "Create Rule…");
    assert_eq!(OverflowAction::Settings.label(), "Settings…");
    assert_eq!(OverflowAction::CopyLink.label(), "Copy Link");
}

// PICK-09, PKS-03
#[test]
fn the_url_line_shows_only_when_enabled() {
    let link = Url::parse("https://www.github.com/example/repo/pull/42?tab=files").unwrap();
    let source = SourceLabel {
        name: "Slack".into(),
        icon: Some("slack".into()),
    };
    let mut config = shown(&[(app("firefox.desktop"), None)]);
    let off = PickerModel::new(&config, &catalog(), Some(&link), Some(source.clone()));
    assert!(off.url_line.is_none());

    config.picker.show_url = true;
    let on = PickerModel::new(&config, &catalog(), Some(&link), Some(source.clone()));
    let shown_line = on.url_line.unwrap();
    assert_eq!(shown_line.source, Some(source));
    assert_eq!(shown_line.link.host, "github.com");
    assert_eq!(shown_line.full, link.as_str());
    let cut = shown_line.truncated(24);
    assert_eq!(cut.host, "github.com");
    assert!(cut.text().chars().count() <= 24);

    let no_link = PickerModel::new(&config, &catalog(), None, None);
    assert!(no_link.url_line.is_none());
}

// KEY-10
#[test]
fn per_target_hotkeys_are_unique_and_skip_action_keys() {
    let config = shown(&[
        (app("firefox.desktop"), Some("F")),
        (app("brave.desktop"), Some("f")),
        (app("app.zen_browser.zen.desktop"), Some("Return")),
        (work(), Some("Ctrl+w")),
        (app("yandex.desktop"), Some("y")),
    ]);
    // "f" twice: the first keeps it. "Return" is an action key (KEY-12).
    // "Ctrl+w" is not a single key.
    assert_eq!(
        keys(&model(&config)),
        [Some("f".into()), None, None, None, Some("y".into())]
    );
}

// KEY-10
#[test]
fn the_numbers_scheme_numbers_by_position_up_to_nine() {
    let mut config = shown(&[
        (app("firefox.desktop"), Some("z")),
        (app("brave.desktop"), None),
        (work(), None),
    ]);
    config.picker.hotkeys = HotkeyScheme::Numbers;
    assert_eq!(
        keys(&model(&config)),
        [Some("1".into()), Some("2".into()), Some("3".into())]
    );

    let many = catalog();
    let shown_many: Vec<_> = (0..12)
        .map(|_| ShownTarget {
            info: many.describe(&app("firefox.desktop")).unwrap(),
            hotkey: None,
        })
        .collect();
    let assigned = assign_hotkeys(HotkeyScheme::Numbers, &shown_many, &[]);
    assert_eq!(assigned.iter().flatten().count(), MAX_NUMBERED);
    assert!(assigned[9..].iter().all(Option::is_none));
    assert_eq!(assigned[8].as_ref().unwrap().key, "9");
}

// KEY-10
#[test]
fn the_letters_scheme_gives_each_target_the_first_free_letter_of_its_name() {
    let mut config = shown(&[
        (app("firefox.desktop"), None),
        (app("brave.desktop"), None),
        (app("google-chrome.desktop"), None),
        (work(), None),
        (app("app.zen_browser.zen.desktop"), None),
        (app("yandex.desktop"), None),
    ]);
    config.picker.hotkeys = HotkeyScheme::Letters;
    let model = model(&config);
    // Firefox f, Brave b, Google Chrome g, "Work (Google Chrome)" w,
    // Zen z, Яндекс Браузер я (a non-Latin letter stays a letter).
    assert_eq!(
        keys(&model),
        [
            Some("f".into()),
            Some("b".into()),
            Some("g".into()),
            Some("w".into()),
            Some("z".into()),
            Some("я".into()),
        ]
    );
    assert_eq!(model.tiles[0].hotkey.as_ref().unwrap().label, "F");
}

#[test]
fn letters_move_on_when_the_first_letter_is_taken_or_reserved() {
    let mut config = shown(&[
        (app("firefox.desktop"), None),
        (app("brave.desktop"), None),
        (app("google-chrome.desktop"), None),
    ]);
    config.picker.hotkeys = HotkeyScheme::Letters;
    // Make "b" an action key: Brave must take its next free letter, "r".
    config.picker.keys.more = vec!["b".into()];
    let model = model(&config);
    assert_eq!(
        keys(&model),
        [Some("f".into()), Some("r".into()), Some("g".into())]
    );

    // Two targets sharing every letter: the second finds none.
    let clash = shown_with_names(&["Ab", "Ba", "Ab"]);
    let assigned = assign_hotkeys(HotkeyScheme::Letters, &clash, &[]);
    assert_eq!(
        assigned
            .iter()
            .map(|h| h.as_ref().map(|h| h.key.as_str()))
            .collect::<Vec<_>>(),
        [Some("a"), Some("b"), None]
    );
}

fn shown_with_names(names: &[&str]) -> Vec<ShownTarget> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| ShownTarget {
            info: TargetInfo {
                target: app(&format!("n{i}.desktop")),
                name: (*name).to_owned(),
                long_name: (*name).to_owned(),
                icon: None,
                badge: None,
                caps: TargetCaps::default(),
            },
            hotkey: None,
        })
        .collect()
}

// KEY-10
#[test]
fn the_off_scheme_shows_no_hotkeys() {
    let mut config = shown(&[(app("firefox.desktop"), Some("f"))]);
    config.picker.hotkeys = HotkeyScheme::Off;
    assert_eq!(keys(&model(&config)), [None]);
}

// KEY-11
#[test]
fn hotkeys_match_case_insensitively() {
    let config = shown(&[
        (app("firefox.desktop"), Some("f")),
        (app("brave.desktop"), Some("B")),
    ]);
    let model = model(&config);
    let hotkeys = model.hotkeys();
    assert_eq!(find_hotkey(&hotkeys, &press("f", &[])), Some(0));
    assert_eq!(
        find_hotkey(&hotkeys, &press("F", &[Modifier::Shift])),
        Some(0)
    );
    assert_eq!(find_hotkey(&hotkeys, &press("b", &[])), Some(1));
    assert_eq!(find_hotkey(&hotkeys, &press("x", &[])), None);
}

// KEY-11
#[test]
fn non_latin_layouts_match_the_character_or_the_key_position() {
    let config = shown(&[
        (app("firefox.desktop"), Some("a")),
        (app("yandex.desktop"), Some("я")),
    ]);
    let hotkeys = model(&config).hotkeys();
    // Russian layout, physical KEY_A (evdev 30 → Qt 38) produces "ф".
    let on_key_a = KeyEvent {
        key: Some("Cyrillic_ef".into()),
        text: Some("ф".into()),
        native_scancode: Some(38),
        modifiers: Modifiers::NONE,
    };
    assert_eq!(
        find_hotkey(&hotkeys, &on_key_a),
        Some(0),
        "Latin key at that position"
    );
    // The hotkey "я" is matched by the character the layout produces.
    let typed_ya = KeyEvent {
        key: Some("Cyrillic_ya".into()),
        text: Some("я".into()),
        native_scancode: Some(44),
        modifiers: Modifiers::NONE,
    };
    assert_eq!(find_hotkey(&hotkeys, &typed_ya), Some(1));
}

// KEY-22
#[test]
fn default_keys_dispatch_to_their_actions() {
    let keymap = PickerKeymap::new(&PickerKeys::default());
    assert!(keymap.problems().is_empty());
    let action =
        |key: &str, modifiers: &[Modifier]| match keymap.dispatch(&press(key, modifiers), &[]) {
            KeyOutcome::Action { action, mode } => Some((action, mode)),
            _ => None,
        };
    assert_eq!(action("Return", &[]), Some((PickerAction::Open, None)));
    assert_eq!(action("KP_Enter", &[]), Some((PickerAction::Open, None)));
    assert_eq!(action("space", &[]), Some((PickerAction::Open, None)));
    assert_eq!(action("Escape", &[]), Some((PickerAction::Cancel, None)));
    assert_eq!(action("Right", &[]), Some((PickerAction::Next, None)));
    assert_eq!(action("Tab", &[]), Some((PickerAction::Next, None)));
    assert_eq!(action("Left", &[]), Some((PickerAction::Previous, None)));
    assert_eq!(
        action("ISO_Left_Tab", &[Modifier::Shift]),
        Some((PickerAction::Previous, None))
    );
    assert_eq!(action("Home", &[]), Some((PickerAction::First, None)));
    assert_eq!(action("End", &[]), Some((PickerAction::Last, None)));
    assert_eq!(
        action("c", &[Modifier::Ctrl]),
        Some((PickerAction::CopyLink, None))
    );
    assert_eq!(action("Menu", &[]), Some((PickerAction::More, None)));
    assert_eq!(
        action("r", &[Modifier::Ctrl]),
        Some((PickerAction::CreateRule, None))
    );
    assert_eq!(action("z", &[]), None);
}

// KEY-13, PICK-33
#[test]
fn held_modifiers_ride_along_with_actions_and_hotkeys() {
    let config = shown(&[(app("firefox.desktop"), Some("f"))]);
    let model = model(&config);
    let keymap = PickerKeymap::new(&config.picker.keys);
    let hotkeys = model.hotkeys();
    let outcome =
        |key: &str, modifiers: &[Modifier]| keymap.dispatch(&press(key, modifiers), &hotkeys);

    assert_eq!(
        outcome("Return", &[Modifier::Shift]),
        KeyOutcome::Action {
            action: PickerAction::Open,
            mode: Some(OpenMode::Private)
        }
    );
    assert_eq!(
        outcome("Return", &[Modifier::Ctrl]),
        KeyOutcome::Action {
            action: PickerAction::Open,
            mode: Some(OpenMode::Background)
        }
    );
    assert_eq!(
        outcome("Return", &[Modifier::Alt]),
        KeyOutcome::Action {
            action: PickerAction::Open,
            mode: Some(OpenMode::NewWindow)
        }
    );
    assert_eq!(
        outcome("f", &[]),
        KeyOutcome::Hotkey {
            index: 0,
            mode: None
        }
    );
    assert_eq!(
        outcome("F", &[Modifier::Shift]),
        KeyOutcome::Hotkey {
            index: 0,
            mode: Some(OpenMode::Private)
        }
    );
    assert_eq!(
        outcome("f", &[Modifier::Alt]),
        KeyOutcome::Hotkey {
            index: 0,
            mode: Some(OpenMode::NewWindow)
        }
    );
    // Exact bindings beat the held-modifier reading: Ctrl+C copies.
    assert_eq!(
        outcome("c", &[Modifier::Ctrl]),
        KeyOutcome::Action {
            action: PickerAction::CopyLink,
            mode: None
        }
    );
    // A set that is no held-modifier action ignores the key (KEY-05).
    assert_eq!(outcome("f", &[Modifier::Super]), KeyOutcome::Ignored);
    assert_eq!(
        outcome("f", &[Modifier::Shift, Modifier::Ctrl]),
        KeyOutcome::Ignored
    );
    // The modifier press itself.
    assert_eq!(outcome("Shift_L", &[Modifier::Shift]), KeyOutcome::Ignored);
    assert_eq!(
        keymap.mode_for(mods(&[Modifier::Shift])),
        Some(OpenMode::Private)
    );
    assert_eq!(keymap.mode_for(Modifiers::NONE), None);
}

// KEY-22
#[test]
fn custom_keys_replace_the_defaults() {
    let keys = PickerKeys {
        cancel: vec!["q".into(), "Escape".into()],
        background_modifier: mods(&[Modifier::Super]),
        ..PickerKeys::default()
    };
    let keymap = PickerKeymap::new(&keys);
    assert_eq!(
        keymap.dispatch(&press("q", &[]), &[]),
        KeyOutcome::Action {
            action: PickerAction::Cancel,
            mode: None
        }
    );
    assert_eq!(
        keymap.mode_for(mods(&[Modifier::Super])),
        Some(OpenMode::Background)
    );
    assert_eq!(keymap.mode_for(mods(&[Modifier::Ctrl])), None);
    assert_eq!(keymap.bindings(PickerAction::Cancel).len(), 2);
}

#[test]
fn unreadable_bindings_are_reported_and_left_out() {
    let keys = PickerKeys {
        open: vec!["Return".into(), "Hyper+x".into()],
        ..PickerKeys::default()
    };
    let keymap = PickerKeymap::new(&keys);
    assert_eq!(keymap.problems().len(), 1);
    assert_eq!(keymap.problems()[0].action, PickerAction::Open);
    assert_eq!(keymap.problems()[0].text, "Hyper+x");
    assert_eq!(keymap.bindings(PickerAction::Open).len(), 1);
}

// KEY-12, KEY-21
#[test]
fn action_keys_block_hotkeys_and_report_who_uses_them() {
    let keymap = PickerKeymap::new(&PickerKeys::default());
    let plain = keymap.plain_keys();
    for key in [
        "Return", "KP_Enter", "space", "Escape", "Right", "Tab", "Left", "Home", "End", "Menu",
    ] {
        assert!(plain.contains(&key.to_owned()), "{key}");
    }
    assert!(!plain.contains(&"c".to_owned()), "Ctrl+c needs a modifier");
    assert_eq!(
        keymap.action_blocking_hotkey("Return"),
        Some(PickerAction::Open)
    );
    assert_eq!(
        keymap.action_blocking_hotkey("escape"),
        Some(PickerAction::Cancel)
    );
    assert_eq!(keymap.action_blocking_hotkey("x"), None);
    assert_eq!(keymap.action_blocking_hotkey("Ctrl+x"), None);
    let ctrl_r = "Ctrl+r".parse().unwrap();
    assert_eq!(keymap.action_using(&ctrl_r), Some(PickerAction::CreateRule));
    assert_eq!(PickerAction::CreateRule.label(), "Create rule from link…");
    assert_eq!(keymap.action_using(&"Ctrl+x".parse().unwrap()), None);
}

// PICK-22
#[test]
fn selection_wraps_and_jumps() {
    assert_eq!(select(PickerAction::Next, 0, 3), 1);
    assert_eq!(select(PickerAction::Next, 2, 3), 0);
    assert_eq!(select(PickerAction::Previous, 0, 3), 2);
    assert_eq!(select(PickerAction::Previous, 2, 3), 1);
    assert_eq!(select(PickerAction::First, 2, 3), 0);
    assert_eq!(select(PickerAction::Last, 0, 3), 2);
    assert_eq!(select(PickerAction::Open, 1, 3), 1);
    assert_eq!(
        select(PickerAction::Open, 7, 3),
        2,
        "a stale index is clamped"
    );
    assert_eq!(select(PickerAction::Next, 0, 0), 0);
}

// KEY-13, PICK-14
#[test]
fn modes_have_hints_and_dim_tiles_that_cannot_comply() {
    assert_eq!(OpenMode::Private.hint(), "Open in a private window");
    let config = shown(&[
        (app("firefox.desktop"), None),
        (app("app.zen_browser.zen.desktop"), None),
        (work(), None),
    ]);
    let model = model(&config);
    // Firefox has private windows; Zen here does not; a profile has none.
    assert_eq!(model.dimmed(Some(OpenMode::Private)), [false, true, true]);
    assert_eq!(
        model.dimmed(Some(OpenMode::Background)),
        [false, false, false]
    );
    assert_eq!(
        model.dimmed(Some(OpenMode::NewWindow)),
        [false, false, false]
    );
    assert_eq!(model.dimmed(None), [false, false, false]);
}

// KEY-13, PICK-32, PICK-33
#[test]
fn choosing_applies_the_held_way_of_opening() {
    let config = shown(&[
        (app("firefox.desktop"), None),
        (app("app.zen_browser.zen.desktop"), None),
        (work(), None),
    ]);
    let model = model(&config);

    let plain = model.choose(0, None).unwrap();
    assert_eq!(plain.chosen.target, app("firefox.desktop"));
    assert!(plain.honoured);
    assert_eq!(plain.chosen.options, OpenOptions::default());

    let private = model.choose(0, Some(OpenMode::Private)).unwrap();
    assert_eq!(
        private.chosen.target,
        Target::Private(id("firefox.desktop"))
    );

    let background = model.choose(2, Some(OpenMode::Background)).unwrap();
    assert_eq!(background.chosen.target, work());
    assert!(background.chosen.options.background);
    assert!(!background.chosen.options.new_window);

    let window = model.choose(0, Some(OpenMode::NewWindow)).unwrap();
    assert!(window.chosen.options.new_window);

    // A target that cannot do it opens normally and says so.
    let refused = model.choose(1, Some(OpenMode::Private)).unwrap();
    assert!(!refused.honoured);
    assert_eq!(refused.chosen.target, app("app.zen_browser.zen.desktop"));
    assert_eq!(refused.chosen.options, OpenOptions::default());

    assert!(model.choose(9, None).is_none());
}

// KEY-13: what the service accepts back for a shown tile.
#[test]
fn a_tile_can_come_back_plain_or_as_its_private_target() {
    let config = shown(&[
        (app("firefox.desktop"), None),
        (app("app.zen_browser.zen.desktop"), None),
    ]);
    let model = model(&config);
    assert_eq!(
        choosable(&model.tiles[0].info),
        [
            app("firefox.desktop"),
            Target::Private(id("firefox.desktop"))
        ]
    );
    assert_eq!(
        choosable(&model.tiles[1].info),
        [app("app.zen_browser.zen.desktop")],
        "no private window"
    );
}

#[test]
fn a_private_target_stays_private_under_the_private_key() {
    let catalog = catalog();
    let info = catalog
        .describe(&Target::Private(id("firefox.desktop")))
        .unwrap();
    let choice = choose(&info, Some(OpenMode::Private));
    assert!(choice.honoured);
    assert_eq!(choice.chosen.target, info.target);
}

// PICK-30
#[test]
fn the_tile_menu_offers_what_the_target_supports() {
    let catalog = catalog();
    let labels = |target: &Target| -> Vec<&'static str> {
        tile_menu(&catalog.describe(target).unwrap())
            .into_iter()
            .filter_map(|entry| match entry {
                TileMenuEntry::Action { label, .. } => Some(label),
                TileMenuEntry::Separator => None,
            })
            .collect()
    };
    assert_eq!(
        labels(&app("firefox.desktop")),
        [
            "Open",
            "Open in Private Window",
            "Open in New Window",
            "Open in Background",
            "Make Primary Browser"
        ]
    );
    assert_eq!(
        labels(&app("app.zen_browser.zen.desktop")),
        [
            "Open",
            "Open in New Window",
            "Open in Background",
            "Make Primary Browser"
        ]
    );
    assert_eq!(
        labels(&Target::Private(id("firefox.desktop"))),
        [
            "Open",
            "Open in New Window",
            "Open in Background",
            "Make Primary Browser"
        ]
    );
    let entries = tile_menu(&catalog.describe(&app("firefox.desktop")).unwrap());
    assert_eq!(entries[entries.len() - 2], TileMenuEntry::Separator);
    let custom = Target::Custom(CustomApp::Executable("tool".into()));
    assert_eq!(
        labels(&custom),
        ["Open", "Open in Background", "Make Primary Browser"]
    );
}
