//! Rules files (RUL-02): export every rule with its script, import rules at
//! the end of the list. Blocking file work.
//!
//! A rule's script lives in `rules/<rule-id>.js` next to `config.toml`
//! (12-data-model.md, "Storage"). An imported rule with a script gets a new
//! ID so its script has a file of its own.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use wye_api::Error;
use wye_api::actions::ScriptScope;
use wye_core::Rule;
use wye_core::rules_file::{self, BundledRule, Imported, Problem};

use crate::api::scripts::ScriptFiles;

/// The directory of the rules' scripts, next to `config`.
pub(crate) fn scripts_dir(config: &Path) -> PathBuf {
    ScriptFiles::beside(config).rules_dir()
}

/// RUL-02: the rules and their scripts as a rules file.
///
/// # Errors
///
/// `Failed` when a script cannot be read or the file cannot be written.
pub(crate) fn export(rules: &[Rule], scripts: &Path) -> Result<String, Error> {
    let bundled = rules
        .iter()
        .map(|rule| {
            Ok(BundledRule {
                rule: rule.clone(),
                script: read_script(scripts, rule)?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    rules_file::export(&bundled)
        .map_err(|error| Error::failed(format!("cannot write the rules file: {error}")))
}

/// The script of `rule`, read the way the pipeline reads it: a rule ID
/// that cannot name a file (`../transform`, say) has no script.
fn read_script(scripts: &Path, rule: &Rule) -> Result<Option<String>, Error> {
    let Some(id) = &rule.id else {
        return Ok(None);
    };
    let files = ScriptFiles::in_rules_dir(scripts);
    match files.read(&ScriptScope::Rule(id.clone())) {
        Err(Error::InvalidArgs(reason)) => {
            tracing::warn!(%reason, "exporting a rule without its script");
            Ok(None)
        }
        other => other,
    }
}

/// Read a rules file.
///
/// # Errors
///
/// `InvalidArgs` when the text is not a rules file, or when no rule in it
/// can be imported (the message lists why).
pub(crate) fn read(text: &str) -> Result<Imported, Error> {
    let imported =
        rules_file::import(text).map_err(|error| Error::invalid_args(error.to_string()))?;
    if imported.rules.is_empty() && !imported.skipped.is_empty() {
        let problems: Vec<String> = imported
            .skipped
            .iter()
            .map(|skipped| {
                let problem = match &skipped.problem {
                    Problem::Unreadable(error) => error.clone(),
                    Problem::Invalid(errors) => errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                format!("rule {}: {problem}", skipped.position)
            })
            .collect();
        return Err(Error::invalid_args(problems.join("\n")));
    }
    Ok(imported)
}

/// The imported rules, ready to append to `existing`: a rule with a script
/// gets an ID no other rule or script file uses, and its script is written.
///
/// # Errors
///
/// `Failed` when a script cannot be written.
pub(crate) fn place(
    imported: Vec<BundledRule>,
    existing: &[Rule],
    scripts: &Path,
) -> Result<Vec<Rule>, Error> {
    let mut taken: BTreeSet<String> = existing.iter().filter_map(|rule| rule.id.clone()).collect();
    imported
        .into_iter()
        .map(|bundled| {
            let Some(script) = bundled.script else {
                return Ok(bundled.rule);
            };
            let id = free_id(&taken, scripts);
            taken.insert(id.clone());
            let path = scripts.join(format!("{id}.js"));
            wye_desktop::atomic::write(&path, script.as_bytes(), None).map_err(|error| {
                Error::failed(format!("cannot write {}: {error}", path.display()))
            })?;
            Ok(Rule {
                id: Some(id),
                ..bundled.rule
            })
        })
        .collect()
}

/// `rules` with `rule-<n>` given to every rule without an ID, so each saved
/// rule can own a script file (RUL-25). Rules that have one keep it.
pub(crate) fn with_ids(rules: Vec<Rule>, scripts: &Path) -> Vec<Rule> {
    let mut taken: BTreeSet<String> = rules.iter().filter_map(|rule| rule.id.clone()).collect();
    rules
        .into_iter()
        .map(|rule| {
            if rule.id.is_some() {
                return rule;
            }
            let id = free_id(&taken, scripts);
            taken.insert(id.clone());
            Rule {
                id: Some(id),
                ..rule
            }
        })
        .collect()
}

/// `rule-<n>` for the smallest `n` no rule and no script file uses.
fn free_id(taken: &BTreeSet<String>, scripts: &Path) -> String {
    (1..=u32::MAX)
        .map(|n| format!("rule-{n}"))
        .find(|id| !taken.contains(id) && !scripts.join(format!("{id}.js")).exists())
        .unwrap_or_else(|| "rule".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"
format = "wye-rules"
version = 1

[[rule]]
name = "GitHub"
target = { picker = true }
url-matchers = [{ kind = "domain", pattern = "github.com" }]
transform = true
script = "export default (url) => url;"
"#;

    #[test]
    fn a_rule_and_its_script_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let scripts = dir.path().join("rules");
        let imported = read(FILE).expect("a rules file");
        let rules = place(imported.rules, &[], &scripts).expect("placed");
        assert_eq!(rules[0].id.as_deref(), Some("rule-1"));
        assert!(scripts.join("rule-1.js").is_file());

        let exported = export(&rules, &scripts).expect("exported");
        let again = read(&exported).expect("a rules file");
        assert_eq!(
            again.rules[0].script.as_deref(),
            Some("export default (url) => url;")
        );
        assert_eq!(again.rules[0].rule.name, "GitHub");
    }

    /// RUL-02: an ID that is not a file name never reads another file.
    #[test]
    fn a_rule_id_cannot_reach_outside_the_rules_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let scripts = dir.path().join("rules");
        std::fs::create_dir_all(&scripts).expect("dir");
        std::fs::write(dir.path().join("transform.js"), "secret").expect("written");
        let imported = read(FILE).expect("a rules file");
        let rule = Rule {
            id: Some("../transform".to_owned()),
            ..imported.rules[0].rule.clone()
        };
        assert!(matches!(read_script(&scripts, &rule), Ok(None)));
    }

    #[test]
    fn ids_in_use_are_skipped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let scripts = dir.path().join("rules");
        std::fs::create_dir_all(&scripts).expect("dir");
        std::fs::write(scripts.join("rule-2.js"), "").expect("written");
        let taken = BTreeSet::from(["rule-1".to_owned()]);
        assert_eq!(free_id(&taken, &scripts), "rule-3");
    }

    #[test]
    fn a_file_without_usable_rules_is_invalid() {
        assert!(matches!(read("x = 1"), Err(Error::InvalidArgs(_))));
        let broken = "format = \"wye-rules\"\nversion = 1\n[[rule]]\nenabled = true\n";
        assert!(matches!(read(broken), Err(Error::InvalidArgs(_))));
    }
}
