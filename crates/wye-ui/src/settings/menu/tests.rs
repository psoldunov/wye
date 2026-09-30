use serde_json::json;
use wye_api::AppRef;
use wye_api::services::ServiceInfo;

use super::*;

fn info(target: &Value, kind: TargetKind, name: &str) -> TargetInfo {
    let text =
        json!({"target": target, "kind": kind.as_str(), "name": name, "icon": name.to_lowercase()});
    serde_json::from_value(text).expect("target info")
}

fn with_browser(mut info: TargetInfo, browser: &str) -> TargetInfo {
    info.browser = Some(browser.to_owned());
    if info.kind == TargetKind::Profile {
        info.short_name = Some("Work".to_owned());
    }
    info
}

fn inventory() -> TargetInventory {
    TargetInventory {
        targets: vec![
            info(&json!({"picker": true}), TargetKind::Picker, "Picker"),
            info(
                &json!({"app": "firefox.desktop"}),
                TargetKind::App,
                "Firefox",
            ),
            info(&json!({"app": "chrome.desktop"}), TargetKind::App, "Chrome"),
            info(
                &json!({"app": "alacritty.desktop"}),
                TargetKind::App,
                "Alacritty",
            ),
            info(
                &json!({"app": "discord.desktop"}),
                TargetKind::App,
                "Discord",
            ),
            with_browser(
                info(
                    &json!({"private": "firefox.desktop"}),
                    TargetKind::Private,
                    "Firefox (Private)",
                ),
                "firefox.desktop",
            ),
            with_browser(
                info(
                    &json!({"profile": {"app": "chrome.desktop", "id": "P1"}}),
                    TargetKind::Profile,
                    "Work (Chrome)",
                ),
                "chrome.desktop",
            ),
        ],
    }
}

fn discord() -> ServiceInfo {
    ServiceInfo {
        id: "discord".into(),
        name: "Discord".into(),
        installed_app: Some(AppRef {
            id: "discord.desktop".into(),
            name: "Discord".into(),
            icon: Some("discord".into()),
        }),
        target: json!({"default": true}),
        ..ServiceInfo::default()
    }
}

fn labels(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|row| match row.kind {
            RowKind::Separator => "---".to_owned(),
            _ => row.label.clone(),
        })
        .collect()
}

fn request<'a>(
    surface: Surface,
    current: &'a Value,
    service: Option<&'a ServiceInfo>,
    services: &'a [ServiceInfo],
) -> Request<'a> {
    Request {
        surface,
        current,
        service,
        services,
        primary_name: "Firefox",
    }
}

/// The labels of the Browsers menu with the Picker current, while Discord's
/// service exists.
fn browsers_menu() -> Vec<String> {
    let current = json!({"picker": true});
    let services = [discord()];
    let rows = build(
        &inventory(),
        &request(Surface::Browsers, &current, None, &services),
    );
    labels(&rows)
}

#[test]
fn the_browsers_menu_has_no_default_and_no_own_app() {
    // TGT-02 a and c
    assert_eq!(
        browsers_menu(),
        [
            "Picker",
            "---",
            "Alacritty",
            "Chrome",
            "Firefox",
            "---",
            "Private Browsing",
            "Firefox (Private)",
            "---",
            "Profiles: Chrome",
            "Work",
            "---",
            "Other…",
        ]
    );
}

#[test]
fn the_apps_menu_leads_with_default_and_the_services_own_app() {
    // TGT-02 a, b, c; APP-05: the own app sits right below Default and Picker
    let current = json!({"default": true});
    let services = [discord()];
    let rows = build(
        &inventory(),
        &request(Surface::Apps, &current, Some(&services[0]), &services),
    );
    let names = labels(&rows);
    assert_eq!(
        &names[..5],
        ["Default (Firefox)", "---", "Picker", "---", "Discord"]
    );
    assert!(rows[0].checked, "TGT-03: the current value is checked");
}

#[test]
fn another_services_own_app_is_not_a_browser() {
    // TGT-05: Discord's app is offered to Discord's row only
    assert!(!browsers_menu().contains(&"Discord".to_owned()));
}

#[test]
fn every_menu_offers_the_picker() {
    // TGT-07
    let current = json!({"app": "firefox.desktop"});
    for surface in [Surface::Browsers, Surface::Apps, Surface::Rule] {
        let rows = build(&inventory(), &request(surface, &current, None, &[]));
        assert!(rows.iter().any(|row| row.label == "Picker"), "{surface:?}");
    }
}

#[test]
fn browsers_are_alphabetical() {
    // TGT-05
    let current = json!({"picker": true});
    let rows = build(
        &inventory(),
        &request(Surface::Browsers, &current, None, &[]),
    );
    let apps: Vec<_> = rows
        .iter()
        .filter(|row| row.kind == RowKind::Item && row.target.get("app").is_some())
        .map(|row| row.label.as_str())
        .collect();
    assert_eq!(apps, ["Alacritty", "Chrome", "Discord", "Firefox"]);
}

#[test]
fn only_the_current_value_has_a_checkmark() {
    // TGT-03
    let current = json!({"private": "firefox.desktop"});
    let rows = build(
        &inventory(),
        &request(Surface::Browsers, &current, None, &[]),
    );
    let checked: Vec<_> = rows
        .iter()
        .filter(|row| row.checked)
        .map(|row| row.label.as_str())
        .collect();
    assert_eq!(checked, ["Firefox (Private)"]);
}

#[test]
fn private_and_profile_items_show_their_browsers_icon() {
    // TGT-03: the fixture gives every target its own icon, so clear it
    let mut inventory = inventory();
    for target in &mut inventory.targets {
        if target.browser.is_some() {
            target.icon = None;
        }
    }
    let current = json!({"picker": true});
    let rows = build(&inventory, &request(Surface::Browsers, &current, None, &[]));
    let private = rows
        .iter()
        .find(|row| row.label == "Firefox (Private)")
        .expect("row");
    let profile = rows.iter().find(|row| row.label == "Work").expect("row");
    assert_eq!(private.icon, "firefox");
    assert_eq!(profile.icon, "chrome");
}

#[test]
fn a_current_value_whose_app_is_gone_leads_the_menu_as_missing() {
    // APP-10
    let gone = json!({"app": "gone.desktop"});
    let mut inventory = inventory();
    let mut missing = info(&gone, TargetKind::App, "gone.desktop");
    missing.missing = true;
    inventory.targets.push(missing);
    let rows = build(&inventory, &request(Surface::Apps, &gone, None, &[]));
    let first_item = rows
        .iter()
        .find(|row| {
            row.kind == RowKind::Item && row.label != "Default (Firefox)" && row.label != "Picker"
        })
        .expect("row");
    assert!(first_item.missing && first_item.checked);
}

#[test]
fn the_closed_row_shows_the_current_target() {
    // TGT-01
    let current = json!({"app": "firefox.desktop"});
    let row = describe(
        &inventory(),
        &request(Surface::Browsers, &current, None, &[]),
    );
    assert_eq!(
        (row.label.as_str(), row.icon.as_str()),
        ("Firefox", "firefox")
    );
    let default = json!({"default": true});
    let row = describe(&inventory(), &request(Surface::Apps, &default, None, &[]));
    assert_eq!(row.label, "Default (Firefox)");
}

#[test]
fn an_unknown_target_shows_as_missing() {
    // APP-10
    let current = json!({"app": "ghost.desktop"});
    let row = describe(&inventory(), &request(Surface::Apps, &current, None, &[]));
    assert!(row.missing);
    assert_eq!(row.label, "ghost.desktop");
}

#[test]
fn a_services_installed_app_is_described_even_when_the_inventory_lacks_it() {
    let current = json!({"app": "discord.desktop"});
    let services = [discord()];
    let empty = TargetInventory::default();
    let row = describe(
        &empty,
        &request(Surface::Apps, &current, Some(&services[0]), &services),
    );
    assert_eq!(row.label, "Discord");
    assert!(!row.missing);
}

#[test]
fn surfaces_parse_from_their_names() {
    assert_eq!(Surface::parse("apps"), Some(Surface::Apps));
    assert_eq!(Surface::parse("nope"), None);
}
