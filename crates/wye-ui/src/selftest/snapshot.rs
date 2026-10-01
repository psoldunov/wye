//! `wye-ui --self-test [SURFACE] --snapshots DIR`: the self-test, one case at
//! a time, saving a PNG of every visible window after each case.
//!
//! A dev tool, not a gate: the child renders with the software Quick backend
//! (grabs come back empty on the offscreen platform otherwise) and the
//! desktop's platform theme, and its Qt warnings are ignored. Each case's
//! files are named `<surface>-<NN>-<slug>.png`, plus `-w<M>` for the second
//! and later windows (`cpp/wye_shim.cpp`, `saveWindowSnapshots`).

use std::path::Path;

use serde_json::Value;

use crate::surface::Surface;

use super::fixtures::Case;

/// How long a snapshot child may take: every case waits for its render.
pub const TIMEOUT_SECS: u64 = 300;

/// The platform theme a snapshot child uses unless `QT_QPA_PLATFORMTHEME`
/// names another: the desktop's colour scheme, fonts and icon theme
/// (`kdeglobals`), as in a real session.
const DEFAULT_THEME: &str = "kde";

/// Plugin directories searched before `QT_PLUGIN_PATH` in a snapshot child.
/// The dev shell sets it to its own `plasma-integration`: the desktop's copy
/// of the `kde` theme is usually built against another Qt or libstdc++ and
/// does not load.
const EXTRA_PLUGIN_PATH: &str = "WYE_SNAPSHOT_QT_PLUGIN_PATH";

/// Environment of a snapshot child, on top of the self-test's own.
/// `QT_QUICK_BACKEND` is always `software`: under `QT_QPA_PLATFORM=offscreen`
/// `grabWindow` needs it.
pub fn environment() -> Vec<(&'static str, String)> {
    let var = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
    let theme = var("QT_QPA_PLATFORMTHEME").unwrap_or_else(|| DEFAULT_THEME.to_owned());
    let mut environment = vec![
        ("QT_QUICK_BACKEND", "software".to_owned()),
        ("QT_QPA_PLATFORMTHEME", theme),
    ];
    if let Some(extra) = var(EXTRA_PLUGIN_PATH) {
        let path = match var("QT_PLUGIN_PATH") {
            Some(path) => format!("{extra}:{path}"),
            None => extra,
        };
        environment.push(("QT_PLUGIN_PATH", path));
    }
    environment
}

/// Longest slug in a file name.
const MAX_SLUG: usize = 48;

/// Fields of a JSON argument that name what the case shows, in file-name
/// order.
const NAMING_FIELDS: [&str; 4] = ["page", "sheet", "scope", "scheme"];

/// The path prefix of each case's snapshots: `DIR/<surface>-<NN>-<slug>`.
pub fn prefixes(dir: &Path, surface: Surface, cases: &[Case]) -> Vec<String> {
    cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let stem = format!(
                "{}-{:02}-{}",
                surface.name(),
                index + 1,
                slug(surface, case)
            );
            dir.join(stem).to_string_lossy().into_owned()
        })
        .collect()
}

/// What the case shows, as a file-name part: the action unless `show`, the
/// key unless it is the surface's own name, then the argument (a plain
/// string, or the naming fields of a JSON object).
fn slug(surface: Surface, case: &Case) -> String {
    let action = (case.action != "show").then_some(case.action.as_str());
    let key = (case.key != surface.name()).then_some(case.key.as_str());
    let words: Vec<String> = action
        .into_iter()
        .chain(key)
        .map(str::to_owned)
        .chain(argument_words(&case.argument))
        .collect();
    let slug = sanitize(&words.join("-"));
    if slug.is_empty() {
        sanitize(&case.action)
    } else {
        slug
    }
}

fn argument_words(argument: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(argument) {
        Ok(Value::Object(object)) => NAMING_FIELDS
            .iter()
            .filter_map(|field| object.get(*field).and_then(Value::as_str))
            .map(str::to_owned)
            .collect(),
        Ok(Value::String(text)) => vec![text],
        Ok(_) => Vec::new(),
        Err(_) => vec![argument.to_owned()],
    }
}

/// Lowercase ASCII letters and digits, runs of anything else as one `-`.
fn sanitize(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let short: String = words.join("-").chars().take(MAX_SLUG).collect();
    short.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(action: &str, key: &str, argument: &str) -> Case {
        Case {
            action: action.to_owned(),
            key: key.to_owned(),
            argument: argument.to_owned(),
        }
    }

    #[test]
    fn a_page_argument_names_the_file() {
        let cases = [
            case("show", "settings", "browsers"),
            case(
                "show",
                "settings",
                r#"{"page":"picker","sheet":"picker-keys","fixture":{}}"#,
            ),
            case("show", "rule-editor", r#"{"domain":"example.com"}"#),
            case("show", "settings", r#"{"page":"general","scheme":"dark"}"#),
        ];
        let prefixes = prefixes(Path::new("/tmp/shots"), Surface::Settings, &cases);
        assert_eq!(
            prefixes,
            [
                "/tmp/shots/settings-01-browsers",
                "/tmp/shots/settings-02-picker-picker-keys",
                "/tmp/shots/settings-03-rule-editor",
                "/tmp/shots/settings-04-general-dark",
            ]
        );
    }

    #[test]
    fn other_actions_and_keys_name_the_file() {
        assert_eq!(slug(Surface::Picker, &case("close", "4", "")), "close-4");
        assert_eq!(slug(Surface::TrayMenu, &case("toggle", "", "{}")), "toggle");
        assert_eq!(
            slug(
                Surface::ScriptEditor,
                &case("show", "script-editor", "rule:unsaved")
            ),
            "rule-unsaved"
        );
        assert_eq!(
            slug(Surface::Settings, &case("show", "settings", "")),
            "show"
        );
    }

    #[test]
    fn slugs_are_short_and_plain() {
        let long = "A".repeat(100);
        let slug = sanitize(&format!("--Ünïcode {long}"));
        assert!(slug.len() <= MAX_SLUG);
        assert!(slug.starts_with("n-code-aaa"));
        assert!(!slug.ends_with('-'));
    }
}
