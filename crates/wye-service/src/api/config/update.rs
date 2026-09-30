//! Checking an `UpdateConfig` patch before anything is written (SET-06).
//!
//! The existing contract holds (decision 6): the file is re-serialised with
//! `Config::to_toml`, so a file that is not lossless, not writable or
//! broken is refused, and so is a patch whose result the parser would
//! correct or drop.

use serde_json::Value;
use wye_api::Error;
use wye_core::merge_patch;
use wye_core::{ConfigWarning, Loaded};

use super::cache::{self, Current};

/// A checked change: the text to write and what reading it back gives.
#[derive(Debug)]
pub(crate) struct Checked {
    pub text: String,
    pub loaded: Loaded,
}

/// Why `current` cannot be changed at `base_revision`, if it cannot.
///
/// # Errors
///
/// `Conflict` for a stale revision, `ReadOnly` for a file Wye must not
/// replace, `NotLossless` for a broken file or one saving would lose values
/// of.
pub(crate) fn writable(current: &Current, base_revision: u64) -> Result<(), Error> {
    if base_revision != 0 && base_revision != current.revision {
        return Err(Error::Conflict(format!(
            "the configuration changed (revision {} is now {}); reload and try again",
            base_revision, current.revision
        )));
    }
    let path = current.environment.config.display();
    if !current.writable {
        return Err(Error::ReadOnly(format!(
            "{path} is managed elsewhere (a symlink or read-only file, for example from \
             home-manager); change the setting there"
        )));
    }
    if let Some(error) = &current.error {
        return Err(Error::NotLossless(format!(
            "{path} has an error, so saving would replace it: {error}"
        )));
    }
    if !current.lossless {
        return Err(Error::NotLossless(format!(
            "saving would drop values {path} contains: {}",
            lossy_lines(&current.warnings).join("; ")
        )));
    }
    Ok(())
}

/// Apply `patch` to `current` and check the result.
///
/// # Errors
///
/// `InvalidArgs` listing every problem: a patch that is not an object, a
/// result that is not a configuration, or values the parser would correct.
pub(crate) fn check(current: &Current, patch: &Value) -> Result<Checked, Error> {
    let config = merge_patch::apply_to_config(&current.config, patch)
        .map_err(|error| Error::invalid_args(error.to_string()))?;
    // RUL-25: every saved rule gets an ID, which names its script file.
    let scripts = crate::api::rules::transfer::scripts_dir(&current.environment.config);
    let config = wye_core::Config {
        rules: crate::api::rules::transfer::with_ids(config.rules, &scripts),
        ..config
    };
    let text = config
        .to_toml()
        .map_err(|error| Error::failed(format!("cannot write the configuration: {error}")))?;
    let loaded = cache::parse(&text).map_err(Error::failed)?;
    let problems: Vec<String> = loaded
        .warnings
        .iter()
        .filter(|warning| !current.warnings.contains(warning))
        .map(ToString::to_string)
        .collect();
    if !problems.is_empty() {
        return Err(Error::invalid_args(problems.join("\n")));
    }
    Ok(Checked { text, loaded })
}

/// The warnings that make a file lossy, one line each.
fn lossy_lines(warnings: &[ConfigWarning]) -> Vec<String> {
    let lossy: Vec<String> = warnings
        .iter()
        .filter(|warning| {
            !Loaded {
                config: wye_core::Config::default(),
                warnings: vec![(*warning).clone()],
            }
            .is_lossless()
        })
        .map(ToString::to_string)
        .collect();
    if lossy.is_empty() {
        warnings.iter().map(ToString::to_string).collect()
    } else {
        lossy
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use wye_core::Config;

    use super::*;
    use crate::api::link::Environment;

    fn current(text: &str) -> Current {
        let environment = Environment::from_lookup(|name| {
            (name == "HOME").then(|| std::ffi::OsString::from("/home/u"))
        })
        .expect("home");
        let loaded = cache::parse(text).expect("TOML");
        cache::from_loaded(Arc::new(environment), loaded, true, 3)
    }

    #[test]
    fn a_stale_revision_is_a_conflict() {
        let current = current("");
        assert!(matches!(writable(&current, 2), Err(Error::Conflict(_))));
        assert!(writable(&current, 3).is_ok());
        assert!(writable(&current, 0).is_ok(), "0 skips the check");
    }

    #[test]
    fn a_lossy_file_is_refused() {
        let current = current("surprise = true\n");
        let Err(Error::NotLossless(message)) = writable(&current, 0) else {
            panic!("not refused");
        };
        assert!(message.contains("surprise"), "{message}");
    }

    #[test]
    fn a_read_only_file_is_refused() {
        let current = Current {
            writable: false,
            ..current("")
        };
        assert!(matches!(writable(&current, 0), Err(Error::ReadOnly(_))));
    }

    #[test]
    fn a_patch_changes_one_value() {
        let current = current("");
        let checked =
            check(&current, &json!({"general": {"launch-at-login": false}})).expect("valid");
        assert!(!checked.loaded.config.general.launch_at_login);
        assert_eq!(
            checked.loaded.config.browsers,
            Config::default().browsers,
            "the rest stays"
        );
    }

    #[test]
    fn values_the_parser_would_correct_are_listed() {
        let current = current("");
        let default = merge_patch::target_patch(&wye_core::Target::Default);
        let patch = json!({"browsers": {"primary": default}});
        let Err(Error::InvalidArgs(message)) = check(&current, &patch) else {
            panic!("accepted");
        };
        assert!(message.contains("browsers.primary"), "{message}");
    }

    #[test]
    fn a_wrong_type_is_invalid() {
        let current = current("");
        let patch = json!({"general": {"launch-at-login": "yes"}});
        assert!(matches!(
            check(&current, &patch),
            Err(Error::InvalidArgs(_))
        ));
        assert!(matches!(
            check(&current, &json!([1])),
            Err(Error::InvalidArgs(_))
        ));
    }

    // RUL-25: a saved rule always has an ID to name its script file.
    #[test]
    fn saved_rules_get_ids() {
        let current = current(
            "[[rules]]\nid = \"rule-1\"\nname = \"kept\"\nurl-matchers = [{ pattern = \"a.example\" }]\n",
        );
        let patch = json!({"rules": [
            {"id": "rule-1", "name": "kept", "url-matchers": [{"pattern": "a.example"}]},
            {"name": "new", "url-matchers": [{"pattern": "b.example"}]},
            {"name": "newer", "url-matchers": [{"pattern": "c.example"}]}
        ]});
        let checked = check(&current, &patch).expect("valid");
        let ids: Vec<_> = checked
            .loaded
            .config
            .rules
            .iter()
            .map(|rule| rule.id.as_deref())
            .collect();
        assert_eq!(ids, [Some("rule-1"), Some("rule-2"), Some("rule-3")]);
        assert!(checked.text.contains("id = \"rule-2\""), "{}", checked.text);
    }
}
