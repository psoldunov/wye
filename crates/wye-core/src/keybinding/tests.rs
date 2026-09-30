use super::*;

fn mods(list: &[Modifier]) -> Modifiers {
    Modifiers::from_slice(list)
}

fn binding(text: &str) -> KeyBinding {
    text.parse().unwrap()
}

// KEY-03
#[test]
fn parses_and_stores_in_xkb_form() {
    let cases = [
        ("Ctrl+Shift+O", "Ctrl+Shift+o"),
        ("shift+ctrl+o", "Ctrl+Shift+o"),
        ("Return", "Return"),
        ("KP_Enter", "KP_Enter"),
        ("space", "space"),
        ("Shift+Tab", "Shift+Tab"),
        ("Ctrl+,", "Ctrl+comma"),
        ("Ctrl++", "Ctrl+plus"),
        ("+", "plus"),
        ("Super+Alt+Delete", "Alt+Super+Delete"),
    ];
    let stored: Vec<String> = cases
        .iter()
        .map(|(text, _)| binding(text).stored())
        .collect();
    let expected: Vec<&str> = cases.iter().map(|(_, stored)| *stored).collect();
    assert_eq!(stored, expected);
}

// KEY-03
#[test]
fn shows_in_desktop_style() {
    assert_eq!(binding("Ctrl+Shift+o").label(), "Ctrl+Shift+O");
    assert_eq!(binding("Ctrl+comma").label(), "Ctrl+,");
    assert_eq!(binding("Escape").label(), "Esc");
    assert_eq!(binding("Alt+space").label(), "Alt+Space");
    assert_eq!(binding("Ctrl+q").to_string(), "Ctrl+Q");
}

#[test]
fn every_default_picker_key_round_trips() {
    // KEY-22 defaults.
    let defaults = crate::config::PickerKeys::default();
    let all = [
        &defaults.open,
        &defaults.cancel,
        &defaults.next,
        &defaults.previous,
        &defaults.first,
        &defaults.last,
        &defaults.copy_link,
        &defaults.more,
        &defaults.create_rule,
    ];
    for stored in all.into_iter().flatten() {
        let parsed = binding(stored);
        assert_eq!(&parsed.stored(), stored, "{stored} is stored canonically");
        assert_eq!(binding(&parsed.stored()), parsed);
    }
}

#[test]
fn rejects_what_is_not_a_binding() {
    assert!("".parse::<KeyBinding>().is_err());
    assert!("Ctrl+".parse::<KeyBinding>().is_err());
    assert!(
        "Ctrl+Shift".parse::<KeyBinding>().is_err(),
        "a modifier is not a key"
    );
    assert!("Hyper+a".parse::<KeyBinding>().is_err());
    assert!("Ctrl+two words".parse::<KeyBinding>().is_err());
    assert!(matches!(
        "Hyper+a".parse::<KeyBinding>(),
        Err(BindingError::Modifier(_))
    ));
    assert!(matches!(
        "Ctrl+Shift".parse::<KeyBinding>(),
        Err(BindingError::Key(_))
    ));
}

#[test]
fn serde_uses_the_stored_form() {
    let read: KeyBinding = serde_json::from_str(r#""shift+CTRL+O""#).unwrap();
    assert_eq!(serde_json::to_string(&read).unwrap(), r#""Ctrl+Shift+o""#);
    assert!(serde_json::from_str::<KeyBinding>(r#""Ctrl+""#).is_err());
    assert!(serde_json::from_str::<KeyBinding>("42").is_err());
}

#[test]
fn parse_bindings_reports_what_it_could_not_read() {
    let stored = [
        "Return".to_owned(),
        "Bogus+x".to_owned(),
        "space".to_owned(),
    ];
    let (bindings, errors) = parse_bindings(&stored);
    assert_eq!(bindings.len(), 2);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, "Bogus+x");
}

// KEY-05
#[test]
fn modifiers_must_match_exactly() {
    let shift_tab = binding("Shift+Tab");
    assert!(shift_tab.matches(&KeyEvent::named("Tab", mods(&[Modifier::Shift]))));
    assert!(!shift_tab.matches(&KeyEvent::named("Tab", Modifiers::NONE)));
    assert!(!shift_tab.matches(&KeyEvent::named(
        "Tab",
        mods(&[Modifier::Shift, Modifier::Ctrl])
    )));
    let tab = binding("Tab");
    assert!(tab.matches(&KeyEvent::named("Tab", Modifiers::NONE)));
    assert!(!tab.matches(&KeyEvent::named("Tab", mods(&[Modifier::Shift]))));
}

#[test]
fn key_names_match_case_insensitively_and_through_aliases() {
    let ctrl_shift_o = binding("Ctrl+Shift+o");
    let held = mods(&[Modifier::Ctrl, Modifier::Shift]);
    // With Shift held the layout reports the upper-case keysym.
    assert!(ctrl_shift_o.matches(&KeyEvent::named("O", held)));
    assert!(ctrl_shift_o.matches(&KeyEvent::named("o", held)));
    assert!(!ctrl_shift_o.matches(&KeyEvent::named("p", held)));
    // Shift+Tab arrives as ISO_Left_Tab on X11 and Backtab in Qt.
    let shift = mods(&[Modifier::Shift]);
    assert!(binding("Shift+Tab").matches(&KeyEvent::named("ISO_Left_Tab", shift)));
    assert!(binding("Shift+Tab").matches(&KeyEvent::named("Backtab", shift)));
    assert!(binding("Return").matches(&KeyEvent::named("Return", Modifiers::NONE)));
    assert!(!binding("Return").matches(&KeyEvent::named("KP_Enter", Modifiers::NONE)));
}

// KEY-11
#[test]
fn a_non_latin_layout_matches_the_key_at_the_same_position() {
    // Russian layout: physical KEY_A (evdev 30, Qt scancode 38) makes "ф".
    let event = KeyEvent {
        key: Some("Cyrillic_ef".into()),
        text: Some("ф".into()),
        native_scancode: Some(38),
        modifiers: mods(&[Modifier::Ctrl]),
    };
    assert!(
        binding("Ctrl+a").matches(&event),
        "the Latin key at that position"
    );
    assert!(
        binding("Ctrl+ф").matches(&event),
        "the character the layout produces"
    );
    assert!(!binding("Ctrl+b").matches(&event));
    assert!(
        !binding("a").matches(&event),
        "modifiers still have to be exact"
    );
}

#[test]
fn shifted_characters_match_through_their_position() {
    // Shift+1 produces "!", but the key is still the 1 key.
    let event = KeyEvent {
        key: Some("exclam".into()),
        text: Some("!".into()),
        native_scancode: Some(10),
        modifiers: Modifiers::NONE,
    };
    assert!(event.is_key("1"));
    assert!(!event.is_key("2"));
}

#[test]
fn an_event_without_any_key_information_matches_nothing() {
    let event = KeyEvent::default();
    assert!(!event.is_key("a"));
    assert!(!binding("a").matches(&event));
}
