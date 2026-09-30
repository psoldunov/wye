//! The rules file (RUL-02): what "Export Rules…" writes and "Import Rules…"
//! reads, one TOML file with every rule and its transform script.
//!
//! ```toml
//! format = "wye-rules"
//! version = 1
//!
//! [[rule]]
//! name = "GitHub in Firefox"
//! target = { app = "firefox.desktop" }
//! url-matchers = [{ kind = "domain", pattern = "github.com" }]
//! transform = true
//! script = "export default function transform(url) { … }"
//! ```
//!
//! A rule is written as in the configuration file, without its `id`: IDs are
//! local to an installation (they name the script files), so an import gives
//! each rule a new one. The format is versioned; a newer file is refused
//! rather than half-read.

use serde::{Deserialize, Serialize};

use crate::rule::{Rule, RuleError};

/// The value of the `format` key.
pub const FORMAT: &str = "wye-rules";
/// The version this code reads and writes.
pub const VERSION: u32 = 1;

/// A rule with its transform script (RUL-25), as exported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRule {
    pub rule: Rule,
    /// The text of the rule's script, when it has one.
    pub script: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Header {
    format: String,
    version: u32,
}

#[derive(Serialize)]
struct OutFile<'a> {
    format: &'static str,
    version: u32,
    rule: Vec<OutRule<'a>>,
}

#[derive(Serialize)]
struct OutRule<'a> {
    #[serde(flatten)]
    rule: Rule,
    #[serde(skip_serializing_if = "Option::is_none")]
    script: &'a Option<String>,
}

#[derive(Deserialize)]
struct InRule {
    #[serde(flatten)]
    rule: Rule,
    #[serde(default)]
    script: Option<String>,
}

/// Writes `rules` to a rules file (RUL-02). The rules' IDs are left out.
///
/// # Errors
///
/// Returns an error only if a value cannot be represented in TOML.
pub fn export(rules: &[BundledRule]) -> Result<String, toml::ser::Error> {
    toml::to_string(&OutFile {
        format: FORMAT,
        version: VERSION,
        rule: rules
            .iter()
            .map(|bundled| OutRule {
                rule: Rule {
                    id: None,
                    ..bundled.rule.clone()
                },
                script: &bundled.script,
            })
            .collect(),
    })
}

/// A rule that could not be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Its place in the file, counted from 1.
    pub position: usize,
    /// Its name, when it had one.
    pub name: Option<String>,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The entry is not a rule (a missing name, a value of the wrong type).
    Unreadable(String),
    /// The rule reads but cannot be saved (RUL-18): one message per problem.
    Invalid(Vec<RuleError>),
}

/// The outcome of reading a rules file. Rules that are fine are imported even
/// when others are not, so one bad entry does not cost the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub rules: Vec<BundledRule>,
    pub skipped: Vec<Skipped>,
}

/// A file that is not a usable rules file.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("this is not a TOML file: {0}")]
    NotToml(#[from] toml::de::Error),
    #[error("this is not a Wye rules file")]
    NotRulesFile,
    #[error("this rules file has version {found}; this Wye reads version {VERSION}")]
    Version { found: u32 },
}

/// Reads a rules file (RUL-02).
///
/// Rules come back without IDs, in file order, ready to be added at the end
/// of the list. Every rule is checked as the editor checks it (RUL-18); the
/// ones that fail are listed in [`Imported::skipped`]. A `script` on a rule
/// whose `transform` is off is kept, so turning it on later finds it.
///
/// # Errors
///
/// Returns an error for text that is not TOML, is not a Wye rules file, or
/// has a version this code does not read.
pub fn import(text: &str) -> Result<Imported, ImportError> {
    let table: toml::Table = text.parse()?;
    let header: Header = toml::Value::Table(table.clone())
        .try_into()
        .map_err(|_| ImportError::NotRulesFile)?;
    if header.format != FORMAT {
        return Err(ImportError::NotRulesFile);
    }
    if header.version != VERSION {
        return Err(ImportError::Version {
            found: header.version,
        });
    }
    let entries = table
        .get("rule")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rules = Vec::new();
    let mut skipped = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let position = index + 1;
        let name = entry
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        match read_rule(entry) {
            Ok(rule) => rules.push(rule),
            Err(problem) => skipped.push(Skipped {
                position,
                name,
                problem,
            }),
        }
    }
    Ok(Imported { rules, skipped })
}

fn read_rule(entry: toml::Value) -> Result<BundledRule, Problem> {
    let InRule { rule, script } = entry
        .try_into()
        .map_err(|error: toml::de::Error| Problem::Unreadable(error.message().to_owned()))?;
    let rule = Rule { id: None, ..rule };
    rule.compile().map_err(Problem::Invalid)?;
    Ok(BundledRule { rule, script })
}

#[cfg(test)]
mod tests;
