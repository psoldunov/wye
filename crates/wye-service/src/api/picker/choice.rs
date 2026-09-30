//! Reading `PickerChose`: the target and the `a{sv}` options (PIPE-13,
//! PICK-20, PICK-21, PICK-32, PICK-33, LAUNCH-03).

use wye_api::Error;
use wye_api::context::{
    OPTION_ACTIVATION_TOKEN, OPTION_BACKGROUND, OPTION_NEW_WINDOW, OPTION_PRIVATE,
};
use wye_core::{Chosen, OpenOptions, Target};
use zbus::zvariant::OwnedValue;

use crate::api::Dict;

/// What the picker chose, validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub chosen: Chosen,
    /// The token from the picker's own input event, for the launched app
    /// (PICK-29, LAUNCH-03).
    pub token: Option<String>,
}

/// Parse `target` (configuration JSON) and `options`.
///
/// # Errors
///
/// `InvalidArgs` for a target that is not JSON of a concrete target, or an
/// option of the wrong type.
pub(crate) fn parse(target: &str, options: &Dict) -> Result<Choice, Error> {
    let target: Target = serde_json::from_str(target)
        .map_err(|error| Error::invalid_args(format!("target: {error}")))?;
    if !target.is_concrete() {
        return Err(Error::invalid_args(format!(
            "target: {target} is not something to open"
        )));
    }
    let private = flag(options, OPTION_PRIVATE)?;
    let chosen = Chosen {
        target: private_variant(target, private),
        options: OpenOptions {
            background: flag(options, OPTION_BACKGROUND)?,
            new_window: flag(options, OPTION_NEW_WINDOW)?,
        },
    };
    let token = text(options, OPTION_ACTIVATION_TOKEN)?.filter(|token| !token.is_empty());
    Ok(Choice { chosen, token })
}

/// KEY-13: a private window is the browser's private target. Targets
/// without one open as they are (the UI dims them, PICK-14).
fn private_variant(target: Target, private: bool) -> Target {
    match target {
        Target::App(app) if private => Target::Private(app),
        other => other,
    }
}

fn flag(options: &Dict, key: &str) -> Result<bool, Error> {
    options.get(key).map_or(Ok(false), |value| {
        bool::try_from(value).map_err(|_| wrong_type(key, "b"))
    })
}

fn text(options: &Dict, key: &str) -> Result<Option<String>, Error> {
    options
        .get(key)
        .map(|value: &OwnedValue| {
            <&str>::try_from(value)
                .map(str::to_owned)
                .map_err(|_| wrong_type(key, "s"))
        })
        .transpose()
}

fn wrong_type(key: &str, signature: &str) -> Error {
    Error::invalid_args(format!("option {key} must be of type {signature}"))
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::Value;

    use super::*;

    fn dict(entries: &[(&str, Value<'_>)]) -> Dict {
        entries
            .iter()
            .map(|(key, value)| {
                (
                    (*key).to_owned(),
                    OwnedValue::try_from(value.try_clone().expect("clone")).expect("owned"),
                )
            })
            .collect()
    }

    const FIREFOX: &str = r#"{"app":"firefox.desktop"}"#;

    #[test]
    fn a_plain_choice_opens_the_target_as_it_is() {
        let choice = parse(FIREFOX, &Dict::new()).expect("valid");
        assert_eq!(choice.chosen.target.to_string(), "firefox.desktop");
        assert_eq!(choice.chosen.options, OpenOptions::default());
        assert_eq!(choice.token, None);
    }

    #[test]
    fn held_modifier_options_apply_to_the_choice() {
        // PICK-33: private becomes the private target; background and new
        // window become open options; the token travels (PICK-29).
        let options = dict(&[
            (OPTION_PRIVATE, Value::from(true)),
            (OPTION_BACKGROUND, Value::from(true)),
            (OPTION_ACTIVATION_TOKEN, Value::from("token-7")),
        ]);
        let choice = parse(FIREFOX, &options).expect("valid");
        assert!(matches!(choice.chosen.target, Target::Private(_)));
        assert!(choice.chosen.options.background);
        assert!(!choice.chosen.options.new_window);
        assert_eq!(choice.token.as_deref(), Some("token-7"));
    }

    #[test]
    fn a_profile_ignores_the_private_option() {
        let profile = r#"{"profile":{"app":"google-chrome.desktop","id":"Profile 1"}}"#;
        let options = dict(&[(OPTION_PRIVATE, Value::from(true))]);
        let choice = parse(profile, &options).expect("valid");
        assert!(matches!(choice.chosen.target, Target::Profile { .. }));
    }

    #[test]
    fn the_picker_itself_and_bad_input_are_rejected() {
        for target in [r#"{"picker":true}"#, "not json", r#"{"app":""}"#] {
            let error = parse(target, &Dict::new()).expect_err(target);
            assert!(
                matches!(error, Error::InvalidArgs(_)),
                "{target}: {error:?}"
            );
        }
        let options = dict(&[(OPTION_PRIVATE, Value::from("yes"))]);
        assert!(matches!(
            parse(FIREFOX, &options),
            Err(Error::InvalidArgs(_))
        ));
    }
}
