use serde_json::json;

use super::*;
use crate::target::{CustomApp, DesktopId};

fn id(name: &str) -> DesktopId {
    DesktopId::new(name).unwrap()
}

/// RFC 7386, Appendix A: the test vectors, as (target, patch, result).
fn vectors() -> Vec<(Value, Value, Value)> {
    vec![
        (json!({"a":"b"}), json!({"a":"c"}), json!({"a":"c"})),
        (json!({"a":"b"}), json!({"b":"c"}), json!({"a":"b","b":"c"})),
        (json!({"a":"b"}), json!({"a":null}), json!({})),
        (
            json!({"a":"b","b":"c"}),
            json!({"a":null}),
            json!({"b":"c"}),
        ),
        (json!({"a":["b"]}), json!({"a":"c"}), json!({"a":"c"})),
        (json!({"a":"c"}), json!({"a":["b"]}), json!({"a":["b"]})),
        (
            json!({"a":{"b":"c"}}),
            json!({"a":{"b":"d","c":null}}),
            json!({"a":{"b":"d"}}),
        ),
        (json!({"a":[{"b":"c"}]}), json!({"a":[1]}), json!({"a":[1]})),
        (json!(["a", "b"]), json!(["c", "d"]), json!(["c", "d"])),
        (json!({"a":"b"}), json!(["c"]), json!(["c"])),
        (json!({"a":"foo"}), json!(null), json!(null)),
        (json!({"a":"foo"}), json!("bar"), json!("bar")),
        (json!({"e":null}), json!({"a":1}), json!({"e":null,"a":1})),
        (json!([1, 2]), json!({"a":"b","c":null}), json!({"a":"b"})),
        (
            json!({}),
            json!({"a":{"bb":{"ccc":null}}}),
            json!({"a":{"bb":{}}}),
        ),
    ]
}

#[test]
fn rfc_7386_appendix_a_vectors() {
    for (target, patch, expected) in vectors() {
        assert_eq!(apply(&target, &patch), expected, "{target} + {patch}");
    }
}

#[test]
fn the_target_document_is_not_changed() {
    let target = json!({"a": {"b": 1}, "c": [1, 2]});
    let before = target.clone();
    let patched = apply(&target, &json!({"a": {"b": null, "d": true}, "c": []}));
    assert_eq!(target, before);
    assert_eq!(patched, json!({"a": {"d": true}, "c": []}));
}

#[test]
fn arrays_replace_and_nulls_in_arrays_stay() {
    assert_eq!(
        apply(&json!({"a":[1,2,3]}), &json!({"a":[null,4]})),
        json!({"a":[null,4]})
    );
}

#[test]
fn deleting_a_missing_key_is_fine() {
    assert_eq!(apply(&json!({"a":1}), &json!({"b":null})), json!({"a":1}));
    assert_eq!(apply(&json!(null), &json!({"a":null})), json!({}));
}

#[test]
fn an_empty_patch_changes_nothing() {
    let target = json!({"a":{"b":1}});
    assert_eq!(apply(&target, &json!({})), target);
}

// SET-06: every control sends a merge patch.
#[test]
fn a_patch_changes_one_setting_and_keeps_the_rest() {
    let mut config = Config::default();
    config.picker.show_names = false;
    let patched = apply_to_config(&config, &json!({"picker": {"show-url": true}})).unwrap();
    assert!(patched.picker.show_url);
    assert!(!patched.picker.show_names, "untouched settings stay");
    assert_eq!(patched.browsers, config.browsers);
}

#[test]
fn null_resets_a_setting_to_its_default() {
    let mut config = Config::default();
    config.general.launch_at_login = false;
    config.picker.show_url = true;
    let patched = apply_to_config(
        &config,
        &json!({"general": {"launch-at-login": null}, "picker": null}),
    )
    .unwrap();
    assert!(patched.general.launch_at_login, "the default is on");
    assert_eq!(patched.picker, Config::default().picker);
}

#[test]
fn shortcuts_can_be_set_and_cleared() {
    let config = Config::default();
    let set = apply_to_config(
        &config,
        &json!({"shortcuts": {"toggle-menu": "Ctrl+Alt+o"}}),
    )
    .unwrap();
    assert_eq!(set.shortcuts.toggle_menu.as_deref(), Some("Ctrl+Alt+o"));
    let cleared = apply_to_config(&set, &json!({"shortcuts": {"toggle-menu": null}})).unwrap();
    assert_eq!(cleared.shortcuts.toggle_menu, None);
}

#[test]
fn web_app_mappings_are_added_and_removed_by_key() {
    let config = Config::default();
    let set = apply_to_config(
        &config,
        &json!({"apps": {"spotify": {"app": "spotify.desktop"}}}),
    )
    .unwrap();
    assert_eq!(set.apps["spotify"], Target::App(id("spotify.desktop")));
    let removed = apply_to_config(&set, &json!({"apps": {"spotify": null}})).unwrap();
    assert!(removed.apps.is_empty());
}

#[test]
fn lists_are_replaced_whole() {
    let config = apply_to_config(
        &Config::default(),
        &json!({"browsers": {"shown": [
            {"target": {"app": "firefox.desktop"}, "hotkey": "f"},
            {"target": {"app": "chromium.desktop"}},
        ]}}),
    )
    .unwrap();
    assert_eq!(config.browsers.shown.len(), 2);
    let replaced = apply_to_config(
        &config,
        &json!({"browsers": {"shown": [{"target": {"app": "brave.desktop"}}]}}),
    )
    .unwrap();
    assert_eq!(replaced.browsers.shown.len(), 1);
    assert_eq!(
        replaced.browsers.shown[0].target,
        Target::App(id("brave.desktop"))
    );
    let emptied = apply_to_config(&replaced, &json!({"browsers": {"shown": []}})).unwrap();
    assert!(emptied.browsers.shown.is_empty());
}

#[test]
fn modifiers_are_sent_as_name_lists() {
    let patched = apply_to_config(
        &Config::default(),
        &json!({"browsers": {"alternative-key": ["Ctrl", "Shift"]}}),
    )
    .unwrap();
    assert_eq!(patched.browsers.alternative_key.to_string(), "Shift+Ctrl");
}

#[test]
fn a_patch_that_makes_an_invalid_configuration_is_refused() {
    let config = Config::default();
    for patch in [
        json!({"picker": {"icon-size": "enormous"}}),
        json!({"general": {"launch-at-login": "yes"}}),
        json!({"browsers": {"alternative-key": ["Hyper"]}}),
        json!({"browsers": {"primary": {"app": ""}}}),
    ] {
        assert!(
            matches!(
                apply_to_config(&config, &patch),
                Err(PatchError::Invalid(_))
            ),
            "{patch}"
        );
    }
}

#[test]
fn a_patch_must_be_an_object() {
    let config = Config::default();
    for patch in [json!(null), json!([1]), json!("x"), json!(3)] {
        assert!(matches!(
            apply_to_config(&config, &patch),
            Err(PatchError::NotAnObject)
        ));
    }
}

// A target is a one-key table, which a plain merge would leave with two keys.
#[test]
fn a_plain_merge_of_two_target_kinds_is_not_a_target() {
    // The default primary browser is `{ picker = true }`; merging an app into
    // it leaves a table with two keys.
    let naive = apply_to_config(
        &Config::default(),
        &json!({"browsers": {"primary": {"app": "firefox.desktop"}}}),
    );
    assert!(matches!(naive, Err(PatchError::Invalid(_))));
}

#[test]
fn target_patch_replaces_every_kind_of_target() {
    let targets = [
        Target::Picker,
        Target::App(id("firefox.desktop")),
        Target::Private(id("firefox.desktop")),
        Target::Profile {
            app: id("google-chrome.desktop"),
            id: "Profile 1".into(),
        },
        Target::Custom(CustomApp::Executable("/opt/tool".into())),
        Target::Custom(CustomApp::Desktop(id("tool.desktop"))),
    ];
    let mut config = Config::default();
    for from in &targets {
        for to in &targets {
            config.browsers.primary = from.clone();
            config.browsers.alternative = Target::Picker;
            let patch = json!({"browsers": {"primary": target_patch(to)}});
            let patched = apply_to_config(&config, &patch).unwrap();
            assert_eq!(&patched.browsers.primary, to, "{from} to {to}");
            assert_eq!(patched.browsers.alternative, Target::Picker);
        }
    }
}

#[test]
fn target_patch_names_every_kind() {
    let patch = target_patch(&Target::Picker);
    let object = patch.as_object().unwrap();
    assert_eq!(object.len(), TARGET_KINDS.len());
    assert_eq!(object["picker"], json!(true));
    assert!(
        object
            .iter()
            .filter(|(k, _)| *k != "picker")
            .all(|(_, v)| v.is_null())
    );
    // Every key is one `Target` can serialise to, so none is left un-cleared.
    for target in [
        Target::Picker,
        Target::Default,
        Target::App(id("a.desktop")),
        Target::Private(id("a.desktop")),
        Target::Profile {
            app: id("a.desktop"),
            id: "p".into(),
        },
        Target::Custom(CustomApp::Executable("x".into())),
    ] {
        let value = serde_json::to_value(&target).unwrap();
        let key = value.as_object().unwrap().keys().next().unwrap().clone();
        assert!(TARGET_KINDS.contains(&key.as_str()), "{key}");
    }
}

#[test]
fn a_rule_list_patch_round_trips() {
    let patch = json!({"rules": [{
        "name": "GitHub",
        "target": {"app": "firefox.desktop"},
        "url-matchers": [{"kind": "domain", "pattern": "github.com"}],
        "transform": true,
    }]});
    let config = apply_to_config(&Config::default(), &patch).unwrap();
    assert_eq!(config.rules.len(), 1);
    assert!(config.rules[0].transform);
    assert!(
        config.rules[0].enabled,
        "missing fields take their defaults"
    );
}
