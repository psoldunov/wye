//! Self-test fixtures: `fixtures/<surface>.json`, embedded by `build.rs`.
//!
//! Each file lists the calls the self-test makes on the surface root,
//! exactly as `Main.qml` makes them for a D-Bus call (`crate::route`):
//!
//! ```json
//! {"cases": [{"action": "show", "key": "1", "argument": {"url": {"full": "…"}}}]}
//! ```
//!
//! `argument` may be a string (passed as-is) or any other JSON value
//! (passed as its JSON text, as the D-Bus call would carry it).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::route::Action;
use crate::surface::Surface;

include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));

/// The actions a case may name.
const ACTIONS: [Action; 3] = [Action::Show, Action::Close, Action::Toggle];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFile {
    /// The surface the cases are for, when the file is not named after it
    /// (`fixtures/rules.json` feeds `settings`).
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

/// One call, as QML receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Case {
    pub action: String,
    pub key: String,
    pub argument: String,
}

/// The cases for `surface`.
///
/// # Errors
///
/// When the surface has no fixture, or it is not valid.
pub fn cases(surface: Surface) -> anyhow::Result<Vec<Case>> {
    let name = surface.name();
    anyhow::ensure!(
        FIXTURES.iter().any(|(stem, _)| *stem == name),
        "no fixture fixtures/{name}.json"
    );
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
            cases.push(case(fixture)?);
        }
    }
    Ok(cases)
}

fn case(fixture: FixtureCase) -> anyhow::Result<Case> {
    anyhow::ensure!(
        ACTIONS
            .iter()
            .any(|action| action.as_str() == fixture.action),
        "unknown action {:?}",
        fixture.action
    );
    let argument = match fixture.argument {
        Value::Null => String::new(),
        Value::String(text) => text,
        other => other.to_string(),
    };
    Ok(Case {
        action: fixture.action,
        key: fixture.key,
        argument,
    })
}

#[cfg(test)]
mod tests {
    use wye_api::actions::Window;
    use wye_api::picker::PickerRequest;
    use wye_api::tray::TrayMenu;

    use super::*;

    #[test]
    fn every_surface_has_valid_cases() {
        for surface in Surface::ALL {
            let cases = cases(surface).unwrap_or_else(|error| panic!("{error:#}"));
            assert!(!cases.is_empty());
        }
    }

    #[test]
    fn picker_fixtures_are_real_requests() {
        for case in cases(Surface::Picker).expect("cases") {
            if case.action == "show" {
                wye_api::json::decode::<PickerRequest>("fixture", &case.argument)
                    .unwrap_or_else(|error| panic!("{error}"));
                assert!(!case.key.is_empty(), "a picker request needs an id");
            }
        }
    }

    #[test]
    fn tray_menu_fixtures_are_real_menus() {
        for case in cases(Surface::TrayMenu).expect("cases") {
            wye_api::json::decode::<TrayMenu>("fixture", &case.argument)
                .unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn window_fixtures_name_windows_of_their_surface() {
        let windowed = Surface::ALL
            .into_iter()
            .filter(|surface| !matches!(surface, Surface::Picker | Surface::TrayMenu));
        for surface in windowed {
            for case in cases(surface).expect("cases") {
                let window: Window = case.key.parse().expect("a window name");
                assert_eq!(Surface::for_window(window), surface, "{case:?}");
            }
        }
    }

    #[test]
    fn a_json_argument_travels_as_text() {
        let case = case(FixtureCase {
            action: "show".into(),
            key: String::new(),
            argument: serde_json::json!({"a": 1}),
        })
        .expect("valid");
        assert_eq!(case.argument, r#"{"a":1}"#);
    }
}
