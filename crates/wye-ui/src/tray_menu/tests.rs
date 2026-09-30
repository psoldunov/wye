use super::*;

const MENU: &str = r#"{
  "icon": {"kind": "picker"},
  "visible": true,
  "items": [
    {"id": "header-primary", "kind": "header", "label": "Open links in"},
    {"id": "primary:picker", "kind": "radio", "label": "Picker", "shortcut": "P", "checked": true},
    {"id": "primary:one", "kind": "radio", "label": "One", "shortcut": "1", "icon": "firefox"},
    {"id": "primary:two", "kind": "radio", "label": "Two", "shortcut": "2", "enabled": false},
    {"id": "sep-1", "kind": "separator"},
    {"id": "more", "kind": "submenu", "label": "More", "children": [
      {"id": "history", "kind": "action", "label": "History…"}
    ]},
    {"id": "empty", "kind": "submenu", "label": "Empty"},
    {"id": "settings", "kind": "action", "label": "Settings…", "shortcut": "Ctrl+,"}
  ],
  "placement": {"output": "DP-1", "x": 10, "y": 20}
}"#;

fn view() -> MenuView {
    MenuView::parse(MENU).expect("a menu")
}

#[test]
fn the_payload_is_the_tray_menu_and_the_pointer_tray_08() {
    let view = view();
    assert_eq!(view.rows.len(), 8);
    assert_eq!(
        view.placement,
        Some(Placement {
            output: "DP-1".into(),
            x: 10,
            y: 20,
        })
    );
    let bare = MenuView::parse(r#"{"icon":{"kind":"app"},"visible":true,"items":[]}"#)
        .expect("no placement");
    assert!(bare.placement.is_none());
    assert!(MenuView::parse("[]").is_err());
}

#[test]
fn only_enabled_actions_and_radios_can_be_chosen() {
    let view = view();
    let selectable: Vec<_> = view
        .rows
        .iter()
        .filter(|row| row.selectable)
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(selectable, ["primary:picker", "primary:one", "settings"]);
    assert!(view.is_selectable("history"), "submenu entries count");
    assert!(!view.is_selectable("primary:two"));
    assert!(!view.is_selectable("header-primary"));
    assert!(!view.is_selectable("nothing"));
}

#[test]
fn a_submenu_opens_only_with_entries_tray_15() {
    let view = view();
    let more = view.rows.iter().find(|row| row.id == "more").expect("more");
    assert!(more.opens);
    assert_eq!(more.children[0].id, "history");
    let empty = view
        .rows
        .iter()
        .find(|row| row.id == "empty")
        .expect("empty");
    assert!(!empty.opens);
}

#[test]
fn fixed_accelerators_choose_top_level_items_key_51() {
    let view = view();
    assert_eq!(view.accelerator("p"), Some("primary:picker"));
    assert_eq!(view.accelerator("P"), Some("primary:picker"));
    assert_eq!(view.accelerator("1"), Some("primary:one"));
    assert_eq!(view.accelerator("2"), None, "disabled items do not fire");
    assert_eq!(view.accelerator(","), None, "only single-key shortcuts");
    assert_eq!(view.accelerator(""), None);
    assert_eq!(view.accelerator("pp"), None);
}

#[test]
fn rows_encode_for_qml() {
    let json: serde_json::Value = serde_json::from_str(&view().rows_json()).expect("JSON");
    assert_eq!(json[2]["icon"], "firefox");
    assert_eq!(json[1]["shortcut"], "P");
    assert_eq!(json[4]["kind"], "separator");
    assert_eq!(json[5]["children"][0]["label"], "History…");
}
