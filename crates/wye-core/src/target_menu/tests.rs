use super::*;
use crate::config::{Config, ShownEntry};
use crate::target::{Availability, CustomApp};

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
        icon: Some(format!("{}-icon", name.to_lowercase())),
        browser: true,
        private,
        new_window: private,
        profiles: profiles
            .iter()
            .map(|(id, name)| ProfileEntry {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                badge: Some(Badge::Initial {
                    text: name.chars().take(1).collect(),
                    color: 0x00_66_cc,
                }),
            })
            .collect(),
    }
}

fn catalog() -> TargetCatalog {
    TargetCatalog {
        handlers: vec![
            handler(
                "google-chrome.desktop",
                "Google Chrome",
                true,
                &[("Default", "Personal"), ("Profile 1", "Work")],
            ),
            handler("firefox.desktop", "Firefox", true, &[]),
            HandlerEntry {
                browser: false,
                ..handler("foot.desktop", "foot", false, &[])
            },
            handler("app.zen_browser.zen.desktop", "zen", false, &[]),
        ],
        apps: vec![AppEntry {
            app: id("spotify.desktop"),
            name: "Spotify".into(),
            icon: Some("spotify".into()),
        }],
        custom: vec![CustomEntry {
            app: CustomApp::Executable("/opt/tool/bin/tool".into()),
            name: "Bravo Tool".into(),
            icon: None,
        }],
    }
}

fn spec<'a>(surface: Surface, current: Option<&'a Target>, primary: &'a Target) -> MenuSpec<'a> {
    MenuSpec {
        surface,
        current,
        primary,
        own_apps: &[],
        exclude: &[],
    }
}

fn kinds(menu: &TargetMenu) -> Vec<SectionKind> {
    menu.sections.iter().map(|s| s.kind).collect()
}

fn labels(section: &MenuSection) -> Vec<&str> {
    section.items.iter().map(|i| i.label.as_str()).collect()
}

// TGT-02
#[test]
fn rule_menu_has_every_section_in_order() {
    let primary = app("firefox.desktop");
    let menu = TargetMenu::build(&spec(Surface::Rule, None, &primary), &catalog());
    assert_eq!(
        kinds(&menu),
        [
            SectionKind::Default,
            SectionKind::Picker,
            SectionKind::Apps,
            SectionKind::Private,
            SectionKind::Profiles,
            SectionKind::Other,
        ]
    );
    let headers: Vec<_> = menu.sections.iter().map(|s| s.header.as_deref()).collect();
    assert_eq!(
        headers,
        [
            None,
            None,
            None,
            Some("Private Browsing"),
            Some("Profiles: Google Chrome"),
            None
        ]
    );
    assert_eq!(labels(&menu.sections[0]), ["Default (Firefox)"]);
    assert_eq!(labels(&menu.sections[1]), ["Picker"]);
    assert_eq!(labels(&menu.sections[5]), ["Other…"]);
}

// TGT-02 d, TGT-05, TGT-06
#[test]
fn apps_are_alphabetical_and_include_non_browsers_and_added_apps() {
    let primary = Target::Picker;
    let menu = TargetMenu::build(&spec(Surface::Browsers, None, &primary), &catalog());
    let apps = &menu.sections[1];
    assert_eq!(apps.kind, SectionKind::Apps);
    assert_eq!(
        labels(apps),
        ["Bravo Tool", "Firefox", "foot", "Google Chrome", "zen"]
    );
    assert_eq!(
        apps.items[0].choice,
        MenuChoice::Target(Target::Custom(CustomApp::Executable(
            "/opt/tool/bin/tool".into()
        )))
    );
}

// TGT-02 e
#[test]
fn private_items_name_their_browser() {
    let primary = Target::Picker;
    let menu = TargetMenu::build(&spec(Surface::Browsers, None, &primary), &catalog());
    let private = menu
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Private)
        .unwrap();
    assert_eq!(
        labels(private),
        ["Firefox (Private)", "Google Chrome (Private)"]
    );
    assert_eq!(private.items[0].icon.as_deref(), Some("firefox-icon"));
}

// TGT-02 f, TRAY-12
#[test]
fn profile_sections_show_only_the_profile_name() {
    let primary = Target::Picker;
    let menu = TargetMenu::build(&spec(Surface::Browsers, None, &primary), &catalog());
    let profiles = menu
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Profiles)
        .unwrap();
    assert_eq!(profiles.header.as_deref(), Some("Profiles: Google Chrome"));
    assert_eq!(labels(profiles), ["Personal", "Work"]);
    assert_eq!(
        profiles.items[1].icon.as_deref(),
        Some("google chrome-icon")
    );
    assert!(profiles.items[1].badge.is_some());
}

// TGT-02: (a) only on the Apps page and in the rule editor; (c) only on Apps.
#[test]
fn surfaces_decide_the_sections() {
    let primary = app("firefox.desktop");
    let own = [id("spotify.desktop")];
    let with = |surface| {
        let mut spec = spec(surface, None, &primary);
        spec.own_apps = &own;
        kinds(&TargetMenu::build(&spec, &catalog()))
    };
    assert_eq!(
        with(Surface::Browsers),
        [
            SectionKind::Picker,
            SectionKind::Apps,
            SectionKind::Private,
            SectionKind::Profiles,
            SectionKind::Other
        ]
    );
    assert_eq!(
        with(Surface::Apps),
        [
            SectionKind::Default,
            SectionKind::Picker,
            SectionKind::OwnApp,
            SectionKind::Apps,
            SectionKind::Private,
            SectionKind::Profiles,
            SectionKind::Other
        ]
    );
    assert!(with(Surface::Rule).contains(&SectionKind::Default));
    assert!(!with(Surface::Rule).contains(&SectionKind::OwnApp));
    assert_eq!(
        with(Surface::Picker),
        [
            SectionKind::Apps,
            SectionKind::Private,
            SectionKind::Profiles,
            SectionKind::Other
        ]
    );
}

// TGT-07
#[test]
fn picker_is_offered_on_every_menu_that_can_hold_it() {
    let primary = app("firefox.desktop");
    for surface in [Surface::Browsers, Surface::Apps, Surface::Rule] {
        let menu = TargetMenu::build(&spec(surface, None, &primary), &catalog());
        assert!(menu.find(&Target::Picker).is_some(), "{surface:?}");
    }
    let menu = TargetMenu::build(&spec(Surface::Picker, None, &primary), &catalog());
    assert!(menu.find(&Target::Picker).is_none());
}

// TGT-02 c: the service's own app only appears when installed.
#[test]
fn own_app_section_needs_the_app_installed() {
    let primary = Target::Picker;
    let mut spec = spec(Surface::Apps, None, &primary);
    let own = [id("spotify.desktop"), id("not-installed.desktop")];
    spec.own_apps = &own;
    let menu = TargetMenu::build(&spec, &catalog());
    let section = menu
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::OwnApp)
        .unwrap();
    assert_eq!(labels(section), ["Spotify"]);

    let own = [id("not-installed.desktop")];
    spec.own_apps = &own;
    let menu = TargetMenu::build(&spec, &catalog());
    assert!(!kinds(&menu).contains(&SectionKind::OwnApp));
}

#[test]
fn an_own_app_that_is_also_a_handler_is_listed_once() {
    let primary = Target::Picker;
    let mut spec = spec(Surface::Apps, None, &primary);
    let own = [id("firefox.desktop")];
    spec.own_apps = &own;
    let menu = TargetMenu::build(&spec, &catalog());
    let count = menu
        .items()
        .filter(|i| i.choice == MenuChoice::Target(app("firefox.desktop")))
        .count();
    assert_eq!(count, 1);
}

// TGT-03
#[test]
fn the_current_value_is_checked_and_nothing_else() {
    let primary = Target::Picker;
    let work = Target::Profile {
        app: id("google-chrome.desktop"),
        id: "Profile 1".into(),
    };
    for current in [
        Target::Picker,
        Target::Default,
        app("firefox.desktop"),
        Target::Private(id("firefox.desktop")),
        work.clone(),
    ] {
        let menu = TargetMenu::build(&spec(Surface::Rule, Some(&current), &primary), &catalog());
        let checked: Vec<_> = menu.items().filter(|i| i.checked).collect();
        assert_eq!(checked.len(), 1, "{current}");
        assert_eq!(checked[0].choice, MenuChoice::Target(current));
        assert!(!checked[0].missing);
    }
}

// APP-10, TGT-01
#[test]
fn a_missing_current_app_stays_visible_and_checked() {
    let primary = Target::Picker;
    let gone = app("removed.desktop");
    let menu = TargetMenu::build(&spec(Surface::Rule, Some(&gone), &primary), &catalog());
    let item = menu.checked().unwrap();
    assert!(item.missing);
    assert_eq!(item.label, "removed.desktop");
    assert_eq!(item.choice, MenuChoice::Target(gone));
}

#[test]
fn excluded_targets_are_left_out_and_empty_sections_vanish() {
    let primary = Target::Picker;
    let exclude = [
        app("firefox.desktop"),
        Target::Private(id("firefox.desktop")),
        Target::Private(id("google-chrome.desktop")),
    ];
    let mut spec = spec(Surface::Picker, None, &primary);
    spec.exclude = &exclude;
    let menu = TargetMenu::build(&spec, &catalog());
    assert!(menu.find(&app("firefox.desktop")).is_none());
    assert!(!kinds(&menu).contains(&SectionKind::Private));
    assert!(menu.find(&app("google-chrome.desktop")).is_some());
}

#[test]
fn an_empty_catalogue_still_offers_picker_and_other() {
    let primary = Target::Picker;
    let menu = TargetMenu::build(
        &spec(Surface::Browsers, Some(&Target::Picker), &primary),
        &TargetCatalog::default(),
    );
    assert_eq!(kinds(&menu), [SectionKind::Picker, SectionKind::Other]);
}

#[test]
fn default_item_names_a_picker_primary() {
    let primary = Target::Picker;
    let menu = TargetMenu::build(&spec(Surface::Rule, None, &primary), &catalog());
    assert_eq!(menu.items().next().unwrap().label, "Default (Picker)");
}

// SHOWN-02, TRAY-12
#[test]
fn describe_gives_short_and_long_names() {
    let catalog = catalog();
    let work = Target::Profile {
        app: id("google-chrome.desktop"),
        id: "Profile 1".into(),
    };
    let info = catalog.describe(&work).unwrap();
    assert_eq!(info.name, "Work");
    assert_eq!(info.long_name, "Work (Google Chrome)");
    assert_eq!(info.icon.as_deref(), Some("google chrome-icon"));
    assert!(info.caps.new_window);

    let private = catalog
        .describe(&Target::Private(id("firefox.desktop")))
        .unwrap();
    assert_eq!(private.name, "Firefox (Private)");
    assert!(private.caps.private);

    assert!(catalog.describe(&Target::Picker).is_none());
    assert!(catalog.describe(&Target::Default).is_none());
    assert!(catalog.describe(&app("nope.desktop")).is_none());
    // Zen has no private-window support, so no private target exists.
    assert!(
        catalog
            .describe(&Target::Private(id("app.zen_browser.zen.desktop")))
            .is_none()
    );
    assert!(
        catalog
            .describe(&Target::Profile {
                app: id("google-chrome.desktop"),
                id: "Profile 9".into()
            })
            .is_none()
    );
}

#[test]
fn the_catalogue_answers_availability() {
    let catalog = catalog();
    assert!(catalog.is_available(&app("firefox.desktop")));
    assert!(catalog.is_available(&app("spotify.desktop")));
    assert!(!catalog.is_available(&app("nope.desktop")));
}

#[test]
fn all_lists_apps_then_private_then_profiles() {
    let names: Vec<_> = catalog().all().into_iter().map(|i| i.name).collect();
    assert_eq!(
        names,
        [
            "Bravo Tool",
            "Firefox",
            "foot",
            "Google Chrome",
            "zen",
            "Firefox (Private)",
            "Google Chrome (Private)",
            "Personal",
            "Work"
        ]
    );
}

#[test]
fn catalogue_round_trips_through_json() {
    let catalog = catalog();
    let json = serde_json::to_string(&catalog).unwrap();
    assert_eq!(
        serde_json::from_str::<TargetCatalog>(&json).unwrap(),
        catalog
    );
}

// PICK-03, TRAY-11: the shown list, in the user's order.
#[test]
fn shown_targets_keep_the_users_order_and_hotkeys() {
    let mut config = Config::default();
    config.browsers.shown = vec![
        ShownEntry {
            target: app("firefox.desktop"),
            hotkey: Some("f".into()),
        },
        ShownEntry {
            target: app("removed.desktop"),
            hotkey: Some("r".into()),
        },
        ShownEntry {
            target: Target::Profile {
                app: id("google-chrome.desktop"),
                id: "Profile 1".into(),
            },
            hotkey: None,
        },
    ];
    let shown = shown_targets(&config, &catalog());
    let names: Vec<_> = shown.iter().map(|s| s.info.name.as_str()).collect();
    assert_eq!(names, ["Firefox", "Work"], "the removed app is skipped");
    assert_eq!(shown[0].hotkey.as_deref(), Some("f"));
    assert_eq!(shown[1].hotkey, None);
}

#[test]
fn with_nothing_shown_the_installed_browsers_are() {
    let shown = shown_targets(&Config::default(), &catalog());
    let names: Vec<_> = shown.iter().map(|s| s.info.name.as_str()).collect();
    assert_eq!(names, ["Firefox", "Google Chrome", "zen"]);
    assert!(shown.iter().all(|s| s.hotkey.is_none()));
}

#[test]
fn shown_entries_that_are_all_gone_do_not_fall_back() {
    let mut config = Config::default();
    config.browsers.shown = vec![ShownEntry {
        target: app("removed.desktop"),
        hotkey: None,
    }];
    assert!(shown_targets(&config, &catalog()).is_empty());
}
