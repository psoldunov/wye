use wye_api::context::Modifier as WireModifier;
use wye_core::config::ShownEntry;
use wye_core::target_menu::{HandlerEntry, ProfileEntry};
use wye_core::{DesktopId, Modifier};

use super::*;

fn id(text: &str) -> DesktopId {
    DesktopId::new(text).expect("desktop id")
}

fn handler(app: &str, name: &str, private: bool, profiles: Vec<ProfileEntry>) -> HandlerEntry {
    HandlerEntry {
        app: id(app),
        name: name.to_owned(),
        icon: Some(app.trim_end_matches(".desktop").to_owned()),
        browser: true,
        private,
        new_window: true,
        profiles,
    }
}

fn catalog() -> TargetCatalog {
    TargetCatalog {
        handlers: vec![
            handler("firefox.desktop", "Firefox", true, Vec::new()),
            handler(
                "google-chrome.desktop",
                "Google Chrome",
                true,
                vec![ProfileEntry {
                    id: "Profile 1".into(),
                    name: "Work".into(),
                    badge: Some(Badge::Initial {
                        text: "W".into(),
                        color: 0x33_66_99,
                    }),
                }],
            ),
            handler("brave.desktop", "Brave", false, Vec::new()),
        ],
        apps: Vec::new(),
        custom: Vec::new(),
    }
}

fn work() -> Target {
    Target::Profile {
        app: id("google-chrome.desktop"),
        id: "Profile 1".into(),
    }
}

fn config() -> Config {
    let mut config = Config::default();
    config.browsers.shown = vec![
        ShownEntry {
            target: Target::App(id("firefox.desktop")),
            hotkey: Some("F".into()),
        },
        ShownEntry {
            target: work(),
            hotkey: None,
        },
    ];
    config
}

fn url() -> Url {
    Url::parse("https://www.github.com/psoldunov/wye?tab=readme").expect("url")
}

fn request(config: &Config, held: Modifiers, preview: bool) -> PickerRequest {
    build(&Input {
        config,
        catalog: &catalog(),
        url: &url(),
        source: Some(SourceLabel {
            name: "Slack".into(),
            icon: Some("slack".into()),
        }),
        held,
        placement: Some(Placement {
            output: "DP-1".into(),
            x: 10,
            y: 20,
        }),
        preview,
    })
}

#[test]
fn tiles_follow_the_shown_browsers_with_canonical_hotkeys() {
    // PICK-03, PICK-04, KEY-10: hotkeys travel as key names.
    let request = request(&config(), Modifiers::NONE, false);
    let names: Vec<_> = request
        .tiles
        .iter()
        .map(|tile| tile.name.as_str())
        .collect();
    assert_eq!(names.len(), 2);
    assert_eq!(request.tiles[0].hotkey.as_deref(), Some("f"));
    assert_eq!(
        request.tiles[0].target,
        serde_json::json!({"app": "firefox.desktop"})
    );
    assert!(request.tiles[0].capabilities.private);
    assert!(request.tiles[0].capabilities.background);
}

#[test]
fn a_profile_tile_carries_its_badge_as_a_colour() {
    // PICK-06, PKS-04.
    let request = request(&config(), Modifiers::NONE, false);
    assert_eq!(
        request.tiles[1].badge,
        Some(WireBadge::Initial {
            initial: "W".into(),
            color: "#336699".into(),
        })
    );
    let mut hidden = config();
    hidden.picker.show_profile_badge = false;
    assert_eq!(request_badge(&hidden), None);
}

fn request_badge(config: &Config) -> Option<WireBadge> {
    request(config, Modifiers::NONE, false).tiles[1]
        .badge
        .clone()
}

#[test]
fn the_overflow_menu_offers_what_is_not_a_tile() {
    // PICK-08, PICK-28.
    let request = request(&config(), Modifiers::NONE, false);
    let offered: Vec<_> = request
        .overflow
        .iter()
        .flat_map(|group| group.tiles.iter())
        .map(|tile| tile.target.clone())
        .collect();
    assert!(offered.contains(&serde_json::json!({"app": "brave.desktop"})));
    assert!(!offered.contains(&serde_json::json!({"app": "firefox.desktop"})));
}

#[test]
fn keys_travel_under_their_configuration_names() {
    // KEY-22, KEY-13.
    let request = request(&config(), Modifiers::NONE, false);
    assert_eq!(
        request.keys.actions.get("open"),
        Some(&vec![
            "Return".to_owned(),
            "KP_Enter".to_owned(),
            "space".to_owned()
        ])
    );
    assert_eq!(
        request.keys.actions.get("create-rule"),
        Some(&vec!["Ctrl+r".to_owned()])
    );
    assert_eq!(
        request.keys.modifier_actions.get("new-window"),
        Some(&vec![WireModifier::Alt])
    );
    assert_eq!(request.keys.actions.len(), ACTIONS.len());
}

#[test]
fn url_settings_held_keys_and_placement_are_carried() {
    let held = Modifiers::from_slice(&[Modifier::Shift]);
    let request = request(&config(), held, true);
    assert_eq!(request.url.host, "github.com");
    assert_eq!(request.url.rest, "/psoldunov/wye?tab=readme");
    assert_eq!(
        request.source.map(|source| source.name).as_deref(),
        Some("Slack")
    );
    assert_eq!(request.held, vec![WireModifier::Shift]);
    assert_eq!(request.placement.map(|p| p.output).as_deref(), Some("DP-1"));
    assert!(request.preview, "PKS-06");
    assert_eq!(request.settings.icon_size, icon_size(IconSize::default()));
}
