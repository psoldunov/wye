//! One control's change as an RFC 7386 merge patch (SET-06).
//!
//! Every control on a page builds a small patch and sends it with
//! `UpdateConfig`. Arrays replace, so a list (the shown browsers, the
//! dismissed callouts) is sent whole. A target must go through
//! [`wye_core::merge_patch::target_patch`]: the configuration holds a target
//! as a table with one key (`{app = …}`), and a plain patch would leave the
//! old key next to the new one.

use serde_json::{Map, Value, json};
use wye_core::merge_patch::target_patch;
use wye_core::{Modifier, Modifiers, Target};

/// Why a control's value cannot become a patch.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PatchError {
    /// The key path is empty or has a segment that is not a configuration
    /// key (`general.launch-at-login`).
    #[error("{0:?} is not a configuration key path")]
    Path(String),
    /// The text is not JSON.
    #[error("not valid JSON: {0}")]
    Json(String),
    /// The value is not a target.
    #[error("not a target: {0}")]
    Target(String),
    /// The name is not a modifier.
    #[error("{0}")]
    Modifier(String),
}

/// `{"a": {"b": value}}` for the path `a.b`.
fn nest(path: &str, value: Value) -> Result<Value, PatchError> {
    let segments: Vec<&str> = path.split('.').collect();
    let valid = |segment: &&str| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if !segments.iter().all(valid) {
        return Err(PatchError::Path(path.to_owned()));
    }
    Ok(segments.iter().rev().fold(value, |inner, segment| {
        let mut object = Map::new();
        object.insert((*segment).to_owned(), inner);
        Value::Object(object)
    }))
}

/// Parse the JSON text a QML control sends.
///
/// # Errors
///
/// [`PatchError::Json`] when `text` is not JSON.
pub fn parse(text: &str) -> Result<Value, PatchError> {
    serde_json::from_str(text).map_err(|error| PatchError::Json(error.to_string()))
}

/// Set the key at `path` to `value`, for switches, radio groups and text
/// entries. `null` removes the key, which returns it to its default.
///
/// # Errors
///
/// [`PatchError::Path`] for a path that is not a key path.
pub fn set(path: &str, value: Value) -> Result<Value, PatchError> {
    nest(path, value)
}

fn parse_target(spec: &Value) -> Result<Target, PatchError> {
    serde_json::from_value(spec.clone()).map_err(|error| PatchError::Target(error.to_string()))
}

/// Set the target at `path` (`browsers.primary`, `browsers.alternative`).
///
/// # Errors
///
/// [`PatchError::Target`] when `spec` is not a target, [`PatchError::Path`]
/// for a bad path.
pub fn set_target(path: &str, spec: &Value) -> Result<Value, PatchError> {
    nest(path, target_patch(&parse_target(spec)?))
}

/// Map web service `service` to `spec` (APP-04, APP-06). Default is not
/// stored (12-data-model.md): the mapping is removed.
///
/// # Errors
///
/// [`PatchError::Target`] when `spec` is not a target, [`PatchError::Path`]
/// for an empty service ID.
pub fn set_service_target(service: &str, spec: &Value) -> Result<Value, PatchError> {
    if service.is_empty() {
        return Err(PatchError::Path(service.to_owned()));
    }
    let target = parse_target(spec)?;
    let mapping = if target == Target::Default {
        Value::Null
    } else {
        target_patch(&target)
    };
    Ok(json!({ "apps": { service: mapping } }))
}

/// Set the modifier chooser at `path` (KEY-01): the pressed buttons, none
/// for off.
///
/// # Errors
///
/// [`PatchError::Modifier`] for a name that is not Shift, Ctrl, Alt or
/// Super.
pub fn set_modifiers(path: &str, names: &[String]) -> Result<Value, PatchError> {
    let pressed = names
        .iter()
        .map(|name| {
            name.parse::<Modifier>()
                .map_err(|error| PatchError::Modifier(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let set = Modifiers::from_slice(&pressed);
    let value = serde_json::to_value(set).map_err(|error| PatchError::Json(error.to_string()))?;
    nest(path, value)
}

/// The default value of the section at `path` (`picker.keys`), as the
/// service's `GetDefaults` would say: for "Reset to Defaults" without a
/// service (KEY-04).
///
/// # Errors
///
/// [`PatchError::Path`] when the configuration has no such section.
pub fn local_defaults(path: &str) -> Result<Value, PatchError> {
    let defaults = serde_json::to_value(wye_core::Config::default())
        .map_err(|error| PatchError::Json(error.to_string()))?;
    path.split('.')
        .try_fold(&defaults, |value, key| value.get(key))
        .cloned()
        .ok_or_else(|| PatchError::Path(path.to_owned()))
}

/// The `UpdateUiState` patch that hides callout `id` for good (BLK-09).
/// `dismissed` is the list now; arrays replace, so the whole list goes.
#[must_use]
pub fn dismiss_callout(dismissed: &[String], id: &str) -> Value {
    let mut all = dismissed.to_vec();
    if !all.iter().any(|existing| existing == id) {
        all.push(id.to_owned());
    }
    json!({ "dismissedCallouts": all })
}

/// The `UpdateUiState` patch that remembers the last page (SET-08).
#[must_use]
pub fn last_page(page: &str) -> Value {
    json!({ "lastPage": page })
}

#[cfg(test)]
mod tests {
    use wye_core::Config;
    use wye_core::merge_patch::apply_to_config;

    use super::*;

    fn applied(patch: &Value) -> Config {
        apply_to_config(&Config::default(), patch).expect("a valid configuration")
    }

    #[test]
    fn a_switch_sets_its_key() {
        // GEN-01
        let patch = set("general.launch-at-login", json!(false)).expect("patch");
        assert_eq!(patch, json!({"general": {"launch-at-login": false}}));
        assert!(!applied(&patch).general.launch_at_login);
    }

    #[test]
    fn a_radio_group_sets_a_kebab_case_value() {
        // GEN-02
        let patch = set("general.tray-icon", json!("wye")).expect("patch");
        assert_eq!(
            applied(&patch).general.tray_icon,
            wye_core::config::TrayIcon::Wye
        );
    }

    #[test]
    fn a_bad_path_is_refused() {
        for path in ["", "a..b", "General.x", "a.b c", "a.'b"] {
            assert_eq!(set(path, json!(1)), Err(PatchError::Path(path.to_owned())));
        }
    }

    #[test]
    fn a_target_change_leaves_one_key_in_the_table() {
        // BRW-01: the old `picker` key must go when `app` arrives.
        let patch =
            set_target("browsers.primary", &json!({"app": "firefox.desktop"})).expect("patch");
        let config = applied(&patch);
        assert_eq!(
            config.browsers.primary,
            Target::App(wye_core::DesktopId::new("firefox.desktop").expect("id"))
        );
        let before = json!({"browsers": {"primary": {"picker": true}}});
        let merged = wye_core::merge_patch::apply(&before, &patch);
        assert_eq!(
            merged,
            json!({"browsers": {"primary": {"app": "firefox.desktop"}}})
        );
    }

    #[test]
    fn a_profile_target_keeps_its_id() {
        // TGT-02 f
        let spec = json!({"profile": {"app": "google-chrome.desktop", "id": "Profile 1"}});
        let patch = set_target("browsers.alternative", &spec).expect("patch");
        assert!(matches!(
            applied(&patch).browsers.alternative,
            Target::Profile { .. }
        ));
    }

    #[test]
    fn something_that_is_not_a_target_is_refused() {
        let result = set_target("browsers.primary", &json!({"nonsense": 1}));
        assert!(matches!(result, Err(PatchError::Target(_))), "{result:?}");
    }

    #[test]
    fn a_service_mapping_to_a_target_is_stored() {
        // APP-06
        let patch =
            set_service_target("discord", &json!({"app": "com.discordapp.Discord.desktop"}))
                .expect("patch");
        let config = applied(&patch);
        assert!(matches!(config.apps.get("discord"), Some(Target::App(_))));
    }

    #[test]
    fn a_service_mapping_back_to_default_is_removed() {
        // APP-04: only non-Default mappings are stored.
        let patch = set_service_target("discord", &json!({"default": true})).expect("patch");
        assert_eq!(patch, json!({"apps": {"discord": null}}));
        let before = json!({"apps": {"discord": {"app": "a.desktop"}, "zoom": {"picker": true}}});
        let merged = wye_core::merge_patch::apply(&before, &patch);
        assert_eq!(merged, json!({"apps": {"zoom": {"picker": true}}}));
    }

    #[test]
    fn a_service_mapping_replaces_the_previous_kind() {
        let patch = set_service_target("zoom", &json!({"picker": true})).expect("patch");
        let before = json!({"apps": {"zoom": {"app": "zoom.desktop"}}});
        assert_eq!(
            wye_core::merge_patch::apply(&before, &patch),
            json!({"apps": {"zoom": {"picker": true}}})
        );
    }

    #[test]
    fn a_modifier_chooser_writes_the_pressed_set() {
        // KEY-01, BRW-03
        let names = vec!["Ctrl".to_owned(), "Shift".to_owned()];
        let patch = set_modifiers("browsers.alternative-key", &names).expect("patch");
        assert_eq!(
            patch,
            json!({"browsers": {"alternative-key": ["Shift", "Ctrl"]}})
        );
        assert_eq!(
            applied(&patch).browsers.alternative_key,
            Modifiers::from_slice(&[wye_core::Modifier::Shift, wye_core::Modifier::Ctrl])
        );
    }

    #[test]
    fn no_pressed_button_means_off() {
        // KEY-01
        let patch = set_modifiers("browsers.alternative-key", &[]).expect("patch");
        assert_eq!(patch, json!({"browsers": {"alternative-key": []}}));
        assert!(applied(&patch).browsers.alternative_key.is_empty());
    }

    #[test]
    fn an_unknown_modifier_is_refused() {
        let result = set_modifiers("browsers.alternative-key", &["Hyper".to_owned()]);
        assert!(matches!(result, Err(PatchError::Modifier(_))), "{result:?}");
    }

    #[test]
    fn dismissing_a_callout_keeps_the_others_and_never_repeats() {
        // BLK-09
        let patch = dismiss_callout(&["a".to_owned()], "b");
        assert_eq!(patch, json!({"dismissedCallouts": ["a", "b"]}));
        let again = dismiss_callout(&["a".to_owned(), "b".to_owned()], "b");
        assert_eq!(again, json!({"dismissedCallouts": ["a", "b"]}));
    }

    #[test]
    fn a_section_resets_to_the_defaults() {
        // KEY-04
        let defaults = local_defaults("picker.keys").expect("a section");
        assert_eq!(defaults["cancel"], json!(["Escape"]));
        let reset = set("picker.keys", defaults).expect("patch");
        let mut config = Config::default();
        config.picker.keys.cancel = vec!["q".to_owned()];
        assert_eq!(
            apply_to_config(&config, &reset)
                .expect("config")
                .picker
                .keys
                .cancel,
            ["Escape"]
        );
    }

    #[test]
    fn a_section_that_does_not_exist_has_no_defaults() {
        assert_eq!(
            local_defaults("nope.keys"),
            Err(PatchError::Path("nope.keys".to_owned()))
        );
    }

    #[test]
    fn the_last_page_is_remembered() {
        // SET-08
        assert_eq!(last_page("apps"), json!({"lastPage": "apps"}));
    }

    #[test]
    fn json_text_is_parsed_or_refused() {
        assert_eq!(parse("[1]"), Ok(json!([1])));
        assert!(matches!(parse("{"), Err(PatchError::Json(_))));
    }
}
