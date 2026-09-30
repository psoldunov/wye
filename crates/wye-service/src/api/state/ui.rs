//! The `uiState` part of `Status` and `UpdateUiState` (SET-08, BLK-09,
//! RUL-19, ONB-06), kept in `state.toml`.

use serde_json::Value;
use wye_api::Error;
use wye_api::status::UiState;
use wye_core::merge_patch;
use wye_desktop::State;

/// The UI's view of `state`.
pub(crate) fn from_state(state: &State) -> UiState {
    UiState {
        onboarding_done: state.onboarding_done,
        dismissed_callouts: state.dismissed_callouts.clone(),
        last_page: state.last_settings_page.clone(),
        help_arrow_seen: state.rules_help_seen,
    }
}

/// `state` with `patch` (RFC 7386, a patch of [`UiState`]) applied; the
/// fields the UI does not see stay.
///
/// # Errors
///
/// `InvalidArgs` when the patch is not an object or the result is not a
/// `UiState`.
pub(crate) fn patched(state: &State, patch: &Value) -> Result<State, Error> {
    if !patch.is_object() {
        return Err(Error::invalid_args(
            "a UI state patch must be a JSON object",
        ));
    }
    let current = serde_json::to_value(from_state(state))
        .map_err(|error| Error::failed(format!("cannot encode the UI state: {error}")))?;
    let ui: UiState = serde_json::from_value(merge_patch::apply(&current, patch))
        .map_err(|error| Error::invalid_args(format!("UI state: {error}")))?;
    Ok(State {
        onboarding_done: ui.onboarding_done,
        dismissed_callouts: ui.dismissed_callouts,
        last_settings_page: ui.last_page.filter(|page| !page.is_empty()),
        rules_help_seen: ui.help_arrow_seen,
        ..state.clone()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wye_core::DesktopId;

    use super::*;

    #[test]
    fn a_patch_changes_only_what_it_names() {
        let state = State {
            previous_default_browser: DesktopId::new("firefox.desktop").ok(),
            dismissed_callouts: vec!["general-browser-links".to_owned()],
            ..State::default()
        };
        let patch = json!({"lastPage": "rules", "onboardingDone": true});
        let next = patched(&state, &patch).expect("valid");
        assert_eq!(next.last_settings_page.as_deref(), Some("rules"));
        assert!(next.onboarding_done);
        assert_eq!(next.dismissed_callouts, state.dismissed_callouts);
        assert_eq!(
            next.previous_default_browser, state.previous_default_browser,
            "fields outside uiState stay"
        );
    }

    #[test]
    fn null_clears_a_value() {
        let state = State {
            last_settings_page: Some("rules".to_owned()),
            ..State::default()
        };
        let next = patched(&state, &json!({"lastPage": null})).expect("valid");
        assert_eq!(next.last_settings_page, None);
    }

    #[test]
    fn a_wrong_type_is_invalid() {
        let state = State::default();
        assert!(matches!(
            patched(&state, &json!({"onboardingDone": "yes"})),
            Err(Error::InvalidArgs(_))
        ));
        assert!(matches!(
            patched(&state, &json!(true)),
            Err(Error::InvalidArgs(_))
        ));
    }
}
