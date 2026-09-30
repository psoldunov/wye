//! Rules files (RUL-02): export every rule with its script, import rules at
//! the end of the list. Blocking file work.
//!
//! A rule's script lives in `rules/<rule-id>.js` next to `config.toml`
//! (12-data-model.md, "Storage"). An imported rule with a script gets a new
//! ID so its script has a file of its own.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use wye_api::Error;
use wye_core::Rule;
use wye_core::rules_file::{self, BundledRule, Imported, Problem};

/// The directory of the rules' scripts, next to `config`.
pub(crate) fn scripts_dir(config: &Path) -> PathBuf {
    config
        .parent()
        .map_or_else(|| PathBuf::from("rules"), |dir| dir.join("rules"))
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

fn read_script(scripts: &Path, rule: &Rule) -> Result<Option<String>, Error> {
    let Some(id) = &rule.id else {
        return Ok(None);
    };
    let path = scripts.join(format!("{id}.js"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::failed(format!(
            "cannot read {}: {error}",
            path.display()
        ))),
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
