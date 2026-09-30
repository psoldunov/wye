//! The held-keys setting, when a probe is worth running, and the setting's
//! switch around a probe (KEY-06, BRW-03, RUL-27, ADV-11).

use std::sync::Arc;

use wye_core::keys::Modifiers;
use wye_core::rule::Rule;

use super::*;
use crate::platform::fake::{FAKE, FakeModifiers};

#[test]
fn the_setting_is_read_leniently() {
    assert_eq!(held_keys_in(""), HeldKeys::Auto);
    assert_eq!(
        held_keys_in("[advanced]\nheld-keys = \"off\"\n"),
        HeldKeys::Off
    );
    assert_eq!(
        held_keys_in("[advanced]\nheld-keys = \"auto\"\n"),
        HeldKeys::Auto
    );
    assert_eq!(
        held_keys_in("[advanced]\nheld-keys = \"sometimes\"\n"),
        HeldKeys::Auto
    );
    assert_eq!(held_keys_in("[advanced]\nheld-keys = 3\n"), HeldKeys::Auto);
    assert_eq!(held_keys_in("[advanced"), HeldKeys::Auto);
}

#[test]
fn the_default_alternative_key_needs_a_probe_brw_03() {
    assert!(bindings_need_modifiers(
        &Config::default(),
        EntryPoint::Handler
    ));
}

#[test]
fn without_any_modifier_binding_no_probe_is_needed() {
    let mut config = Config::default();
    config.browsers.alternative_key = Modifiers::NONE;
    assert!(!bindings_need_modifiers(&config, EntryPoint::Handler));
    // ADV-11: the bypass key only matters for extension links.
    assert!(bindings_need_modifiers(&config, EntryPoint::Extension));
    config.advanced.force_picker_from_extension = false;
    assert!(!bindings_need_modifiers(&config, EntryPoint::Extension));
}

#[test]
fn an_enabled_rule_with_held_keys_needs_a_probe_rul_27() {
    let mut config = Config::default();
    config.browsers.alternative_key = Modifiers::NONE;
    let rule: Rule = toml::from_str("name = \"Work\"\nheld-keys = [\"Ctrl\"]\n").expect("a rule");
    config.rules = vec![rule.clone()];
    assert!(bindings_need_modifiers(&config, EntryPoint::Clipboard));
    config.rules = vec![Rule {
        enabled: false,
        ..rule
    }];
    assert!(!bindings_need_modifiers(&config, EntryPoint::Clipboard));
}

#[tokio::test]
async fn the_setting_switches_the_probe_per_link() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join("config.toml");
    let fake = Arc::new(FakeModifiers::default());
    fake.set(Some(vec![Modifier::Shift]));
    let probe = Configured::new(fake.clone(), config.clone(), HeldKeys::Auto);

    assert_eq!(probe.mechanism(), Some(FAKE));
    assert_eq!(probe.held().await, Some(vec![Modifier::Shift]));

    std::fs::write(&config, "[advanced]\nheld-keys = \"off\"\n").expect("written");
    assert_eq!(probe.held().await, None);
    assert_eq!(
        probe.mechanism(),
        None,
        "KEY-06: choosers disabled while off"
    );

    std::fs::write(&config, "[advanced]\nheld-keys = \"auto\"\n").expect("written");
    assert_eq!(probe.held().await, Some(vec![Modifier::Shift]));
    assert_eq!(probe.mechanism(), Some(FAKE));
}

#[tokio::test]
async fn a_probe_that_starts_off_reports_no_mechanism() {
    let probe = Configured::new(
        Arc::new(FakeModifiers::default()),
        "/nonexistent/config.toml".into(),
        HeldKeys::Off,
    );
    assert_eq!(probe.mechanism(), None);
}
