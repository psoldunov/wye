//! Self-test fixtures: `fixtures/<name>.json`, embedded by `build.rs`.
//!
//! Each file lists the `ShowWindow` calls the self-test makes, exactly as
//! the D-Bus interface would make them (`crate::route`):
//!
//! ```json
//! {"cases": [{"action": "show", "key": "settings", "argument": {"page": "general", "fixture": {…}}}]}
//! ```
//!
//! `key` is the window name; `argument` may be a string (passed as-is) or
//! any other JSON value (passed as its JSON text, as the D-Bus call would
//! carry it). A file feeds the surface it is named after, or the one its
//! `surface` field names (`fixtures/rules.json` could feed `settings`).
//!
//! Same format as crates/wye-ui/src/selftest/fixtures.rs, so fixture files
//! can be copied between the two.

use serde::Deserialize;
use serde_json::Value;
use wye_api::actions::Window;

use crate::route::{Action, Route};
use crate::surface::Surface;

include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFile {
    /// The surface the cases are for, when the file is not named after it.
    #[serde(default)]
    surface: Option<String>,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureCase {
    action: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    argument: Value,
}

/// One call, as the surface receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub action: Action,
    pub key: String,
    pub argument: String,
}

impl Case {
    /// The route the D-Bus interface would have built.
    #[must_use]
    pub fn route(&self, surface: Surface) -> Route {
        Route {
            surface,
            action: self.action,
            key: self.key.clone(),
            argument: self.argument.clone(),
        }
    }
}

/// The cases for `surface`.
///
/// # Errors
///
/// When the surface has no fixture, or one is not valid.
pub fn cases(surface: Surface) -> anyhow::Result<Vec<Case>> {
    let name = surface.name();
    let mut cases = Vec::new();
    for (stem, text) in FIXTURES {
        let file: FixtureFile = serde_json::from_str(text)
            .map_err(|error| anyhow::anyhow!("fixtures/{stem}.json: {error}"))?;
        let ours = *stem == name || file.surface.as_deref() == Some(name);
        if !ours {
            continue;
        }
        anyhow::ensure!(!file.cases.is_empty(), "fixtures/{stem}.json has no cases");
        for fixture in file.cases {
            cases.push(case(surface, fixture)?);
        }
    }
    anyhow::ensure!(!cases.is_empty(), "no fixture fixtures/{name}.json");
    Ok(cases)
}

fn case(surface: Surface, fixture: FixtureCase) -> anyhow::Result<Case> {
    let action = [Action::Show, Action::Close, Action::Toggle]
        .into_iter()
        .find(|action| action.as_str() == fixture.action)
        .ok_or_else(|| anyhow::anyhow!("unknown action {:?}", fixture.action))?;
    if surface == Surface::Kit {
        // The gallery has no window name of its own.
        anyhow::ensure!(fixture.key == surface.name(), "the kit's key is \"kit\"");
    } else if matches!(surface, Surface::Picker | Surface::TrayMenu) {
        // `PickerHost1` calls: the key is a request ID (or empty), not a
        // window name.
    } else {
        let window: Window = fixture
            .key
            .parse()
            .map_err(|error| anyhow::anyhow!("key {:?}: {error}", fixture.key))?;
        anyhow::ensure!(
            Surface::for_window(window) == surface,
            "{:?} is not a window of {}",
            fixture.key,
            surface.name()
        );
    }
    let argument = match fixture.argument {
        Value::Null => String::new(),
        Value::String(text) => text,
        other => other.to_string(),
    };
    Ok(Case {
        action,
        key: fixture.key,
        argument,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_has_valid_cases() {
        for surface in Surface::ALL {
            let cases = cases(surface).unwrap_or_else(|error| panic!("{error:#}"));
            assert!(!cases.is_empty());
        }
    }

    #[test]
    fn a_json_argument_travels_as_text() {
        let case = case(
            Surface::Settings,
            FixtureCase {
                action: "show".into(),
                key: "settings".into(),
                argument: serde_json::json!({"a": 1}),
            },
        )
        .expect("valid");
        assert_eq!(case.argument, r#"{"a":1}"#);
    }

    #[test]
    fn a_case_names_a_window_of_its_surface() {
        let wrong = case(
            Surface::About,
            FixtureCase {
                action: "show".into(),
                key: "settings".into(),
                argument: Value::Null,
            },
        );
        assert!(wrong.is_err());
    }
}
