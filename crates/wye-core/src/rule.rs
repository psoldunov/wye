//! User rules ([08-rules.md](../../../docs/spec/08-rules.md)).

use serde::{Deserialize, Serialize};

use crate::keys::Modifiers;
use crate::matcher::{CompiledMatcher, MatcherError, UrlMatcher};
use crate::normalize::MatchUrl;
use crate::source::{SourceApp, SourceAppSpec};
use crate::target::Target;

/// Where a rule runs relative to the web app mappings (RUL-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunPosition {
    #[default]
    Before,
    After,
}

impl RunPosition {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Before => "before built-in rules",
            Self::After => "after built-in rules",
        }
    }
}

/// A rule as stored in the configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per option in the rule editor"
)]
pub struct Rule {
    /// Stable ID; names the rule's script file (`rules/<id>.js`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(default = "target_default")]
    pub target: Target,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub url_matchers: Vec<UrlMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_apps: Vec<SourceAppSpec>,
    /// RUL-27: exact modifier set that must be held.
    #[serde(default, skip_serializing_if = "no_keys")]
    pub held_keys: Modifiers,
    #[serde(default)]
    pub open_in_background: bool,
    #[serde(default)]
    pub force_new_window: bool,
    #[serde(default)]
    pub run: RunPosition,
    /// RUL-25: run the rule's transform script when it matches.
    #[serde(default)]
    pub transform: bool,
}

const fn enabled_default() -> bool {
    true
}

const fn target_default() -> Target {
    Target::Default
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde hands `skip_serializing_if` a reference"
)]
const fn no_keys(keys: &Modifiers) -> bool {
    keys.is_empty()
}

/// Why a rule cannot be saved or used (RUL-18).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("the rule has no name")]
    NoName,
    #[error("the rule has no conditions: add a URL matcher, a source app or held keys")]
    NoConditions,
    #[error("URL matcher {index} ({pattern:?}): {source}")]
    Matcher {
        index: usize,
        pattern: String,
        source: MatcherError,
    },
}

impl Rule {
    /// Checks validity (RUL-18) and prepares the matchers.
    ///
    /// # Errors
    ///
    /// Returns every problem found, so the editor can show them all.
    pub fn compile(&self) -> Result<CompiledRule, Vec<RuleError>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(RuleError::NoName);
        }
        if self.url_matchers.is_empty() && self.source_apps.is_empty() && self.held_keys.is_empty()
        {
            errors.push(RuleError::NoConditions);
        }
        let mut matchers = Vec::with_capacity(self.url_matchers.len());
        for (index, matcher) in self.url_matchers.iter().enumerate() {
            match matcher.compile() {
                Ok(compiled) => matchers.push(compiled),
                Err(source) => errors.push(RuleError::Matcher {
                    index: index + 1,
                    pattern: matcher.pattern.clone(),
                    source,
                }),
            }
        }
        if errors.is_empty() {
            Ok(CompiledRule {
                rule: self.clone(),
                matchers,
            })
        } else {
            Err(errors)
        }
    }

    /// A one-line summary for lists: "github.com, gitlab.com · from Slack · Shift"
    /// (RUL-07).
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.url_matchers.is_empty() {
            parts.push(
                self.url_matchers
                    .iter()
                    .map(|m| m.pattern.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !self.source_apps.is_empty() {
            let apps: Vec<_> = self.source_apps.iter().map(ToString::to_string).collect();
            parts.push(format!("from {}", apps.join(", ")));
        }
        if !self.held_keys.is_empty() {
            parts.push(self.held_keys.to_string());
        }
        parts.join(" · ")
    }
}

/// What a rule is tested against.
#[derive(Debug, Clone, Copy)]
pub struct MatchInput<'a> {
    pub url: &'a MatchUrl,
    pub source: &'a SourceApp,
    pub held: Modifiers,
}

/// A validated rule.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule: Rule,
    matchers: Vec<CompiledMatcher>,
}

impl CompiledRule {
    /// RUL-17 and RUL-27: within each list any entry is enough; the lists
    /// that have entries must all match; an empty list does not constrain.
    #[must_use]
    pub fn matches(&self, input: MatchInput<'_>) -> bool {
        let rule = &self.rule;
        rule.enabled
            && (self.matchers.is_empty() || self.matchers.iter().any(|m| m.matches(input.url)))
            && (rule.source_apps.is_empty()
                || rule.source_apps.iter().any(|s| s.matches(input.source)))
            && (rule.held_keys.is_empty() || rule.held_keys.matches(input.held))
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;
    use crate::keys::Modifier;
    use crate::matcher::MatcherKind;
    use crate::target::DesktopId;

    fn rule(matchers: &[&str], sources: &[&str], held: Modifiers) -> Rule {
        Rule {
            id: None,
            name: "Test".into(),
            enabled: true,
            target: Target::Default,
            url_matchers: matchers
                .iter()
                .map(|p| UrlMatcher {
                    kind: MatcherKind::Domain,
                    pattern: (*p).into(),
                })
                .collect(),
            source_apps: sources
                .iter()
                .map(|s| SourceAppSpec::from((*s).to_owned()))
                .collect(),
            held_keys: held,
            open_in_background: false,
            force_new_window: false,
            run: RunPosition::Before,
            transform: false,
        }
    }

    fn check(rule: &Rule, link: &str, source: &SourceApp, held: Modifiers) -> bool {
        let url = MatchUrl::new(&Url::parse(link).unwrap());
        rule.compile().unwrap().matches(MatchInput {
            url: &url,
            source,
            held,
        })
    }

    fn slack() -> SourceApp {
        SourceApp {
            desktop_id: Some(DesktopId::new("com.slack.Slack").unwrap()),
            executable: None,
        }
    }

    #[test]
    fn url_and_source_must_both_match() {
        let r = rule(
            &["github.com", "gitlab.com"],
            &["com.slack.Slack.desktop"],
            Modifiers::NONE,
        );
        assert!(check(&r, "https://gitlab.com/x", &slack(), Modifiers::NONE));
        assert!(!check(
            &r,
            "https://gitlab.com/x",
            &SourceApp::default(),
            Modifiers::NONE
        ));
        assert!(!check(
            &r,
            "https://example.com/",
            &slack(),
            Modifiers::NONE
        ));
    }

    #[test]
    fn empty_lists_do_not_constrain() {
        let r = rule(&[], &["com.slack.Slack.desktop"], Modifiers::NONE);
        assert!(check(
            &r,
            "https://anything.example/",
            &slack(),
            Modifiers::NONE
        ));
    }

    #[test]
    fn held_keys_are_exact_and_combine_with_and() {
        let shift = Modifiers::from_slice(&[Modifier::Shift]);
        let r = rule(&["github.com"], &[], shift);
        assert!(check(
            &r,
            "https://github.com/",
            &SourceApp::default(),
            shift
        ));
        assert!(!check(
            &r,
            "https://github.com/",
            &SourceApp::default(),
            Modifiers::NONE
        ));
        let ctrl_shift = Modifiers::from_slice(&[Modifier::Shift, Modifier::Ctrl]);
        assert!(!check(
            &r,
            "https://github.com/",
            &SourceApp::default(),
            ctrl_shift
        ));
    }

    #[test]
    fn disabled_rules_never_match() {
        let mut r = rule(&["github.com"], &[], Modifiers::NONE);
        r.enabled = false;
        assert!(!check(
            &r,
            "https://github.com/",
            &SourceApp::default(),
            Modifiers::NONE
        ));
    }

    #[test]
    fn validity() {
        let mut r = rule(&[], &[], Modifiers::NONE);
        r.name = " ".into();
        let errors = r.compile().unwrap_err();
        assert!(errors.contains(&RuleError::NoName));
        assert!(errors.contains(&RuleError::NoConditions));

        let bad = Rule {
            url_matchers: vec![UrlMatcher {
                kind: MatcherKind::Regex,
                pattern: "(".into(),
            }],
            ..rule(&[], &[], Modifiers::NONE)
        };
        assert!(matches!(
            bad.compile().unwrap_err()[0],
            RuleError::Matcher { index: 1, .. }
        ));
    }

    #[test]
    fn summary_line() {
        let r = rule(
            &["github.com", "gitlab.com"],
            &["slack"],
            Modifiers::from_slice(&[Modifier::Shift]),
        );
        assert_eq!(r.summary(), "github.com, gitlab.com · from slack · Shift");
    }
}
