//! What QML reads, built from a [`Snapshot`]: the target menu, the shown
//! browsers sheet, the hotkey choices and the help texts, as JSON text. The
//! bridge only converts these strings.

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::services::ServiceInfo;

use super::expansion;
use super::help;
use super::hotkeys;
use super::keys::{self, Check, HotkeyUse};
use super::menu::{self, Request, Surface};
use super::shown::{self, Entry};
use super::snapshot::Snapshot;

/// JSON text for `value`; `[]` when it cannot be encoded (it can always be).
fn text<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot encode a view for QML");
        "[]".to_owned()
    })
}

fn service<'a>(snapshot: &'a Snapshot, id: &str) -> Option<&'a ServiceInfo> {
    snapshot
        .services
        .services
        .iter()
        .find(|service| service.id == id)
}

/// Build something from a menu request for `surface` (`browsers`, `apps`,
/// `rule`) with `current` checked; `service_id` is the mapped web service on
/// the Apps page (TGT-02). `None` for an unknown surface.
fn with_request<T>(
    snapshot: &Snapshot,
    surface: &str,
    current: &Value,
    service_id: &str,
    build: impl FnOnce(&Request<'_>) -> T,
) -> Option<T> {
    let surface = Surface::parse(surface)?;
    let primary_name = snapshot.primary_name();
    let request = Request {
        surface,
        current,
        service: service(snapshot, service_id),
        services: &snapshot.services.services,
        primary_name: &primary_name,
    };
    Some(build(&request))
}

/// The target menu's rows (TGT-02); `[]` for an unknown surface.
#[must_use]
pub fn target_menu(
    snapshot: &Snapshot,
    surface: &str,
    current: &Value,
    service_id: &str,
) -> String {
    with_request(snapshot, surface, current, service_id, |request| {
        text(&menu::build(&snapshot.targets, request))
    })
    .unwrap_or_else(|| "[]".to_owned())
}

/// What the closed target popup row shows for `current` (TGT-01).
#[must_use]
pub fn target_label(
    snapshot: &Snapshot,
    surface: &str,
    current: &Value,
    service_id: &str,
) -> String {
    with_request(snapshot, surface, current, service_id, |request| {
        text(&menu::describe(&snapshot.targets, request))
    })
    .unwrap_or_else(|| "{}".to_owned())
}

/// The shown list as the picker would show it.
#[must_use]
pub fn shown_entries(snapshot: &Snapshot) -> Vec<Entry> {
    let foreign = menu::foreign_app_ids(&snapshot.services.services);
    shown::effective(
        &snapshot.targets,
        &shown::entries(&snapshot.config),
        &foreign,
    )
}

/// The shown browsers sheet's rows (SHOWN-02, SHOWN-03).
#[must_use]
pub fn shown_rows(snapshot: &Snapshot) -> String {
    let foreign = menu::foreign_app_ids(&snapshot.services.services);
    let rows = shown::rows(&snapshot.targets, &shown_entries(snapshot), &foreign);
    let labels = hotkeys::scheme_labels(&snapshot.typed_config(), &rows);
    if labels.is_empty() {
        text(&rows)
    } else {
        text(&shown::with_scheme_hotkeys(rows, &labels))
    }
}

/// The hotkey popup's choices (SHOWN-04).
#[must_use]
pub fn hotkey_choices(snapshot: &Snapshot) -> String {
    text(&hotkeys::choices(&snapshot.typed_config()))
}

/// A recorded hotkey checked against the picker's actions (KEY-12):
/// `{"ok": true, "key": …}` or `{"ok": false, "error": …}`.
#[must_use]
pub fn hotkey_check(snapshot: &Snapshot, recorded: &str) -> String {
    let outcome = match hotkeys::check(&snapshot.typed_config(), recorded) {
        Ok(key) => json!({"ok": true, "key": key}),
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    };
    outcome.to_string()
}

/// The help popover text for `id` (BLK-08), empty for an unknown ID.
#[must_use]
pub fn help_text(snapshot: &Snapshot, id: &str) -> String {
    let key = snapshot.typed_config().browsers.alternative_key;
    let shown = if key.is_empty() {
        "the alternative-browser key".to_owned()
    } else {
        key.to_string()
    };
    let context = help::Context {
        alternative_key: &shown,
        held_keys_available: snapshot.held_keys_available(),
    };
    help::text(id, &context).unwrap_or_default()
}

/// The target hotkeys now set, with their targets' names (KEY-12, KEY-21).
fn hotkey_uses(snapshot: &Snapshot) -> Vec<HotkeyUse> {
    shown_entries(snapshot)
        .into_iter()
        .filter_map(|entry| {
            let key = wye_core::keybinding::canonical_key(entry.hotkey.as_deref()?).ok()?;
            let name = snapshot
                .targets
                .targets
                .iter()
                .find(|info| info.target == entry.target)
                .map_or_else(|| entry.target.to_string(), |info| info.name.clone());
            Some(HotkeyUse { key, name })
        })
        .collect()
}

/// Whether `action` may take the recorded `stored` binding (KEY-21):
/// `{"status": "free" | "duplicate" | "clash" | "invalid", "message": …}`.
#[must_use]
pub fn picker_key_check(snapshot: &Snapshot, action: &str, stored: &str) -> String {
    let outcome = keys::parse(stored).and_then(|binding| {
        keys::check_binding(
            &snapshot.typed_config(),
            &hotkey_uses(snapshot),
            action,
            &binding,
        )
    });
    match outcome {
        Ok(Check::Free) => json!({"status": "free"}),
        Ok(Check::Duplicate) => json!({"status": "duplicate"}),
        Ok(Check::Clash(clash)) => json!({"status": "clash", "message": clash.message()}),
        Err(message) => json!({"status": "invalid", "message": message}),
    }
    .to_string()
}

/// The patch that gives `action` the binding (KEY-02, KEY-21).
///
/// # Errors
///
/// A message for a binding or an action that is not valid.
pub fn picker_key_patch(
    snapshot: &Snapshot,
    action: &str,
    stored: &str,
    replace: bool,
) -> Result<Value, String> {
    let binding = keys::parse(stored)?;
    keys::bind_patch(
        &snapshot.typed_config(),
        &shown_entries(snapshot),
        action,
        &binding,
        replace,
    )
}

fn modifier_set(names: &[String]) -> Result<wye_core::Modifiers, String> {
    let pressed = names
        .iter()
        .map(|name| {
            name.parse::<wye_core::Modifier>()
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(wye_core::Modifiers::from_slice(&pressed))
}

/// Whether the held-modifier action `which` may take the pressed `names`:
/// `{"status": "free" | "clash" | "invalid", "message": …}` (KEY-21).
#[must_use]
pub fn modifiers_check(snapshot: &Snapshot, which: &str, names: &[String]) -> String {
    let outcome = modifier_set(names)
        .and_then(|set| keys::check_modifiers(&snapshot.typed_config(), which, set));
    match outcome {
        Ok(None) => json!({"status": "free"}),
        Ok(Some(clash)) => json!({"status": "clash", "message": clash.message()}),
        Err(message) => json!({"status": "invalid", "message": message}),
    }
    .to_string()
}

/// The patch that gives the held-modifier action `which` the pressed
/// `names` (KEY-01, KEY-21).
///
/// # Errors
///
/// A message for a name or an action that is not valid.
pub fn modifiers_patch(
    snapshot: &Snapshot,
    which: &str,
    names: &[String],
    replace: bool,
) -> Result<Value, String> {
    keys::modifiers_patch(
        &snapshot.typed_config(),
        which,
        modifier_set(names)?,
        replace,
    )
}

/// The URL expansion sheet's rows (DLG-EXP), from what the service shipped
/// and the configuration now.
#[must_use]
pub fn expansion_rows(snapshot: &Snapshot, catalogue: &expansion::Catalogue) -> String {
    text(&expansion::rows(
        catalogue,
        &snapshot.typed_config().advanced.expansion,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        let targets = json!({"targets": [
            {"target": {"picker": true}, "kind": "picker", "name": "Picker"},
            {"target": {"app": "firefox.desktop"}, "kind": "app", "name": "Firefox", "icon": "firefox"}
        ]});
        Snapshot::default()
            .with_config(
                r#"{"browsers":{"primary":{"app":"firefox.desktop"},"alternative-key":["Ctrl"]}}"#,
                1,
            )
            .and_then(|s| s.with_inventory(&targets.to_string(), "{}"))
            .expect("snapshot")
    }

    fn rows(text: &str) -> Vec<Value> {
        serde_json::from_str(text).expect("JSON rows")
    }

    #[test]
    fn the_menu_is_json_rows_with_the_current_value_checked() {
        // TGT-03
        let rows = rows(&target_menu(
            &snapshot(),
            "browsers",
            &json!({"app": "firefox.desktop"}),
            "",
        ));
        let checked: Vec<_> = rows.iter().filter(|row| row["checked"] == true).collect();
        assert_eq!(checked.len(), 1);
        assert_eq!(checked[0]["label"], "Firefox");
    }

    #[test]
    fn an_unknown_surface_has_an_empty_menu() {
        assert_eq!(target_menu(&snapshot(), "nope", &json!({}), ""), "[]");
    }

    #[test]
    fn the_default_item_names_the_primary_browser() {
        // APP-04
        let rows = rows(&target_menu(
            &snapshot(),
            "apps",
            &json!({"default": true}),
            "",
        ));
        assert_eq!(rows[0]["label"], "Default (Firefox)");
    }

    #[test]
    fn the_closed_row_shows_the_target() {
        // TGT-01
        let label: Value = serde_json::from_str(&target_label(
            &snapshot(),
            "browsers",
            &json!({"picker": true}),
            "",
        ))
        .expect("json");
        assert_eq!(label["label"], "Picker");
    }

    #[test]
    fn the_alternative_browser_help_names_the_configured_key() {
        // BRW-02
        assert!(help_text(&snapshot(), "alternative-browser").starts_with("Hold Ctrl while"));
        assert_eq!(help_text(&snapshot(), "nope"), "");
    }

    #[test]
    fn a_reserved_hotkey_is_reported_with_its_action() {
        // KEY-12
        let outcome: Value =
            serde_json::from_str(&hotkey_check(&snapshot(), "Escape")).expect("json");
        assert_eq!(outcome["ok"], false);
        let outcome: Value = serde_json::from_str(&hotkey_check(&snapshot(), "q")).expect("json");
        assert_eq!(outcome, json!({"ok": true, "key": "q"}));
    }

    #[test]
    fn the_shown_sheet_lists_every_installed_browser_when_nothing_is_chosen() {
        // SHOWN-02: an empty list means every installed browser
        let rows = rows(&shown_rows(&snapshot()));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["checked"], true);
    }
}
