//! The hotkey popup of the shown browsers sheet (SHOWN-04, KEY-10, KEY-12).
//!
//! The popup lists "None", a to z and 0 to 9 (keys the picker's own actions
//! use are left out), and "Other Key…", which records any single key.

use serde::Serialize;
use wye_core::Config;
use wye_core::Target;
use wye_core::config::HotkeyScheme;
use wye_core::keybinding::{KeyNameError, canonical_key, display_key};
use wye_core::picker::{PickerKeymap, assign_hotkeys};
use wye_core::target_menu::{ShownTarget, TargetCaps, TargetInfo};

use super::shown::Row;

/// One choice of the popup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    /// The canonical key name that is stored.
    pub key: String,
    /// What the popup shows.
    pub label: String,
}

/// Why a recorded key cannot be a hotkey.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    /// Not a single key.
    #[error("{0}")]
    Key(#[from] KeyNameError),
    /// A picker action uses it (KEY-12).
    #[error("{key} is used by the picker action “{action}”")]
    Reserved { key: String, action: &'static str },
}

fn keymap(config: &Config) -> PickerKeymap {
    PickerKeymap::new(&config.picker.keys)
}

/// The popup's letters and digits, without the keys picker actions use
/// (KEY-12).
#[must_use]
pub fn choices(config: &Config) -> Vec<Choice> {
    let reserved = keymap(config).plain_keys();
    ('a'..='z')
        .chain('0'..='9')
        .map(|c| c.to_string())
        .filter(|key| !reserved.contains(key))
        .map(|key| Choice {
            label: display_key(&key),
            key,
        })
        .collect()
}

/// The canonical name of a key recorded with "Other Key…".
///
/// # Errors
///
/// [`HotkeyError::Key`] when `recorded` is not a single key, and
/// [`HotkeyError::Reserved`] when a picker action uses it (KEY-12).
pub fn check(config: &Config, recorded: &str) -> Result<String, HotkeyError> {
    let key = canonical_key(recorded)?;
    match keymap(config).action_blocking_hotkey(&key) {
        Some(action) => Err(HotkeyError::Reserved {
            key: display_key(&key),
            action: action.label(),
        }),
        None => Ok(key),
    }
}

/// The hotkey each checked row shows under a scheme other than "Assigned
/// per browser": what the core assigns for it (SHOWN-04, KEY-10). Empty for
/// "Assigned per browser", where the popups are live.
#[must_use]
pub fn scheme_labels(config: &Config, rows: &[Row]) -> Vec<Option<String>> {
    if config.picker.hotkeys == HotkeyScheme::PerTarget {
        return Vec::new();
    }
    let shown: Vec<ShownTarget> = rows
        .iter()
        .filter(|row| row.checked)
        .map(|row| ShownTarget {
            info: TargetInfo {
                target: serde_json::from_value(row.target.clone()).unwrap_or(Target::Picker),
                name: row.name.clone(),
                long_name: row.name.clone(),
                icon: None,
                badge: None,
                caps: TargetCaps::default(),
            },
            hotkey: None,
        })
        .collect();
    let reserved = keymap(config).plain_keys();
    assign_hotkeys(config.picker.hotkeys, &shown, &reserved)
        .into_iter()
        .map(|hotkey| hotkey.map(|h| h.label))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::settings::shown::Row;

    fn row(name: &str, checked: bool) -> Row {
        Row {
            key: name.to_owned(),
            target: json!({"app": format!("{name}.desktop")}),
            name: name.to_owned(),
            icon: String::new(),
            badge: None,
            checked,
            hotkey: None,
            shown_hotkey: None,
            removable: false,
            missing: false,
        }
    }

    #[test]
    fn the_popup_offers_letters_and_digits() {
        // SHOWN-04
        let all = choices(&Config::default());
        assert_eq!(all.len(), 36);
        assert_eq!(all[0].key, "a");
        assert_eq!(all[26].key, "0");
    }

    #[test]
    fn a_key_a_picker_action_uses_is_not_offered() {
        // KEY-12
        let mut config = Config::default();
        config.picker.keys.copy_link = vec!["c".to_owned()];
        assert!(choices(&config).iter().all(|choice| choice.key != "c"));
        assert!(
            choices(&Config::default())
                .iter()
                .any(|choice| choice.key == "c")
        );
    }

    #[test]
    fn a_recorded_key_is_canonical() {
        assert_eq!(check(&Config::default(), "F5"), Ok("F5".to_owned()));
        assert_eq!(check(&Config::default(), "A"), Ok("a".to_owned()));
    }

    #[test]
    fn a_recorded_key_that_an_action_uses_names_the_action() {
        // KEY-12: the sheet refuses it and names the action
        let error = check(&Config::default(), "Escape").expect_err("reserved");
        assert!(matches!(error, HotkeyError::Reserved { .. }), "{error:?}");
        assert!(error.to_string().contains("Cancel"), "{error}");
    }

    #[test]
    fn a_modifier_is_not_a_hotkey() {
        assert!(matches!(
            check(&Config::default(), "Shift"),
            Err(HotkeyError::Key(_))
        ));
    }

    /// The hotkeys `scheme` gives the checked rows among `rows`.
    fn under(scheme: HotkeyScheme, rows: &[Row]) -> Vec<Option<String>> {
        let mut config = Config::default();
        config.picker.hotkeys = scheme;
        scheme_labels(&config, rows)
    }

    #[test]
    fn numbers_give_one_to_nine_by_position() {
        // KEY-10
        let rows = [row("Firefox", true), row("Chrome", true), row("Zen", false)];
        assert_eq!(
            under(HotkeyScheme::Numbers, &rows),
            [Some("1".to_owned()), Some("2".to_owned())]
        );
    }

    #[test]
    fn letters_give_each_target_the_first_free_letter() {
        // KEY-10
        let rows = [row("Firefox", true), row("Fennec", true)];
        assert_eq!(
            under(HotkeyScheme::Letters, &rows),
            [Some("F".to_owned()), Some("E".to_owned())]
        );
    }

    #[test]
    fn per_browser_has_no_scheme_labels() {
        assert!(scheme_labels(&Config::default(), &[row("Firefox", true)]).is_empty());
    }
}
