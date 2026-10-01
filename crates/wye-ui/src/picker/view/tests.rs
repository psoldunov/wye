use wye_core::Modifier;
use wye_core::picker::{KeyOutcome, OpenMode, PickerAction};

use super::super::fixture;
use super::*;

#[test]
fn tiles_keep_their_order_hotkeys_and_capabilities() {
    // PICK-03, PICK-04, KEY-10.
    let view = fixture::view();
    assert_eq!(view.tiles.len(), 2);
    assert_eq!(view.tiles[0].name, "Firefox");
    assert_eq!(
        view.tiles[0].hotkey.as_ref().map(|h| h.label.as_str()),
        Some("F")
    );
    assert!(view.tiles[0].info.caps.private);
    assert!(!view.tiles[1].info.caps.private);
    assert_eq!(view.columns(), 2);
}

#[test]
fn large_icons_use_the_large_metrics() {
    // PICK-11.
    let view = fixture::view();
    assert_eq!(
        (view.metrics.icon, view.metrics.pitch, view.metrics.badge),
        (40, 60, 24)
    );
}

#[test]
fn the_keymap_comes_from_the_request() {
    // KEY-22: the request's bindings drive the keymap.
    let view = fixture::view();
    let event = wye_core::keybinding::KeyEvent::named("Escape", wye_core::Modifiers::NONE);
    assert_eq!(
        view.keymap.dispatch(&event, &view.hotkeys()),
        KeyOutcome::Action {
            action: PickerAction::Cancel,
            mode: None
        }
    );
    let shift = wye_core::Modifiers::from_slice(&[Modifier::Shift]);
    assert_eq!(view.keymap.mode_for(shift), Some(OpenMode::Private));
}

#[test]
fn a_tile_with_an_unreadable_target_is_left_out() {
    let mut request = fixture::request();
    request["tiles"][0]["target"] = serde_json::json!({ "app": "" });
    let view = PickerView::parse(&request.to_string()).expect("still shown");
    assert_eq!(view.tiles.len(), 1);
}

#[test]
fn nine_tiles_wrap_after_eight() {
    // PICK-13.
    let mut request = fixture::request();
    let tile = request["tiles"][0].clone();
    request["tiles"] = serde_json::Value::Array(vec![tile; 9]);
    let view = PickerView::parse(&request.to_string()).expect("valid");
    assert_eq!(view.columns(), TILES_PER_ROW);
}

#[test]
fn a_long_link_is_cut_in_the_middle_with_the_host_kept() {
    // PICK-09.
    let mut request = fixture::request();
    let long = format!("https://example.com/{}", "a/".repeat(60));
    request["url"]["full"] = serde_json::json!(long);
    let view = PickerView::parse(&request.to_string()).expect("valid");
    assert_eq!(view.url.host, "example.com");
    assert!(view.url.rest.contains('…'));
    assert!(view.url.host.chars().count() + view.url.rest.chars().count() <= URL_LINE_CHARS);
    assert_eq!(view.url.full, long);
    assert_eq!(view.url.source_name, "Slack");
}

#[test]
fn garbage_is_refused() {
    assert!(PickerView::parse("[").is_err());
}

#[test]
fn a_request_with_unreadable_keys_is_refused() {
    // Refused in `ShowPicker` itself, so the service opens the link through
    // its stand-in rather than waiting for an answer (PICK-23).
    // An action name that collides with a modifier action, with key names
    // where modifiers belong.
    let json = r#"{"keys": {"actions": {"private-modifier": ["Return"]}}}"#;
    assert!(matches!(PickerView::parse(json), Err(ViewError::Keys(_))));
}
