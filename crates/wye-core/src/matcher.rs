//! URL matchers (RUL-14): the five ways a rule can test a link.
//!
//! Every matcher works on the normalised link ([`MatchUrl`]). The examples in
//! `docs/spec/19-help-texts.md` ("URL matchers") are the tests below.

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::normalize::{MatchUrl, normalize_pattern};

/// Upper bound for a compiled user regex, so a pathological pattern cannot
/// eat memory in the link handler.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatcherKind {
    /// The host and its subdomains.
    #[default]
    Domain,
    /// The normalised link starts with the pattern.
    Prefix,
    /// The normalised link contains the pattern.
    Contains,
    /// `*` matches any run of characters; the whole link must match.
    Wildcard,
    /// A regular expression searched in the normalised link.
    Regex,
}

impl MatcherKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Domain => "Domain",
            Self::Prefix => "Starts with",
            Self::Contains => "Contains",
            Self::Wildcard => "Wildcard",
            Self::Regex => "Regular expression",
        }
    }
}

/// A matcher as stored in the configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UrlMatcher {
    #[serde(default)]
    pub kind: MatcherKind,
    pub pattern: String,
}

/// Why a matcher cannot be used; shown inline in the rule editor.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatcherError {
    #[error("the pattern is empty")]
    Empty,
    #[error("a domain matcher takes a host name such as github.com, without a path")]
    DomainWithPath,
    #[error("invalid regular expression: {0}")]
    Regex(String),
}

impl UrlMatcher {
    /// Validates and prepares the matcher.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty pattern, a domain with a path, or an
    /// invalid regular expression.
    pub fn compile(&self) -> Result<CompiledMatcher, MatcherError> {
        let raw = self.pattern.trim();
        if raw.is_empty() {
            return Err(MatcherError::Empty);
        }
        let compiled = match self.kind {
            MatcherKind::Domain => {
                let host = normalize_pattern(raw);
                let host = host.strip_suffix('/').unwrap_or(&host);
                if host.contains(['/', '?', '#']) || host.chars().any(char::is_whitespace) {
                    return Err(MatcherError::DomainWithPath);
                }
                CompiledMatcher::Domain(host.to_owned())
            }
            MatcherKind::Prefix => CompiledMatcher::Prefix(normalize_pattern(raw)),
            // "Contains" often holds a path fragment such as `/pull/`, so it
            // is taken as written: normalising would lowercase it.
            MatcherKind::Contains => CompiledMatcher::Contains(raw.to_owned()),
            MatcherKind::Wildcard => {
                let pattern = normalize_pattern(raw);
                let body = pattern
                    .split('*')
                    .map(regex::escape)
                    .collect::<Vec<_>>()
                    .join(".*");
                CompiledMatcher::Regex(build_regex(&format!("^{body}$"))?)
            }
            MatcherKind::Regex => CompiledMatcher::Regex(build_regex(raw)?),
        };
        Ok(compiled)
    }
}

fn build_regex(pattern: &str) -> Result<Regex, MatcherError> {
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|e| MatcherError::Regex(e.to_string()))
}

/// A validated matcher, ready to test links.
#[derive(Debug, Clone)]
pub enum CompiledMatcher {
    Domain(String),
    Prefix(String),
    Contains(String),
    Regex(Regex),
}

impl CompiledMatcher {
    #[must_use]
    pub fn matches(&self, url: &MatchUrl) -> bool {
        match self {
            Self::Domain(domain) => {
                let host = url.host();
                host == domain
                    || host
                        .strip_suffix(domain.as_str())
                        .is_some_and(|head| head.ends_with('.'))
            }
            Self::Prefix(prefix) => url.as_str().starts_with(prefix.as_str()),
            Self::Contains(needle) => url.as_str().contains(needle.as_str()),
            Self::Regex(re) => re.is_match(url.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    fn matcher(kind: MatcherKind, pattern: &str) -> CompiledMatcher {
        UrlMatcher {
            kind,
            pattern: pattern.into(),
        }
        .compile()
        .unwrap()
    }

    fn hits(m: &CompiledMatcher, link: &str) -> bool {
        m.matches(&MatchUrl::new(
            &Url::parse(&format!("https://{link}")).unwrap(),
        ))
    }

    /// The table in 19-help-texts.md, "URL matchers".
    #[test]
    fn help_text_examples() {
        let cases = [
            (
                MatcherKind::Domain,
                "github.com",
                ["github.com/x", "gist.github.com/y"],
                "notgithub.com",
            ),
            (
                MatcherKind::Prefix,
                "docs.google.com/spreadsheets",
                [
                    "docs.google.com/spreadsheets/d/1",
                    "www.docs.google.com/spreadsheets/d/2",
                ],
                "docs.google.com/document/d/1",
            ),
            (
                MatcherKind::Contains,
                "/pull/",
                ["github.com/a/b/pull/7", "gitlab.com/x/pull/1"],
                "github.com/a/b/issues/7",
            ),
            (
                MatcherKind::Wildcard,
                "*.atlassian.net/browse/*",
                [
                    "team.atlassian.net/browse/ABC-1",
                    "a.b.atlassian.net/browse/X",
                ],
                "atlassian.net/wiki",
            ),
            (
                MatcherKind::Regex,
                r"^meet\.google\.com/[a-z]{3}-",
                ["meet.google.com/abc-defg-hij", "www.meet.google.com/xyz-a"],
                "meet.google.com/landing",
            ),
        ];
        for (kind, pattern, yes, no) in cases {
            let m = matcher(kind, pattern);
            for link in yes {
                assert!(hits(&m, link), "{kind:?} {pattern} should match {link}");
            }
            assert!(!hits(&m, no), "{kind:?} {pattern} should not match {no}");
        }
    }

    #[test]
    fn domain_ignores_case_scheme_and_trailing_slash() {
        let m = matcher(MatcherKind::Domain, "https://www.GitHub.com/");
        assert!(hits(&m, "GITHUB.com/org"));
        assert!(hits(&m, "github.com:8443/org"));
    }

    #[test]
    fn path_comparison_respects_case() {
        let m = matcher(MatcherKind::Prefix, "github.com/Org");
        assert!(hits(&m, "GitHub.com/Org/repo"));
        assert!(!hits(&m, "github.com/org/repo"));
    }

    #[test]
    fn rejects_invalid_patterns() {
        let err = |kind, pattern: &str| {
            UrlMatcher {
                kind,
                pattern: pattern.into(),
            }
            .compile()
            .unwrap_err()
        };
        assert_eq!(err(MatcherKind::Contains, "  "), MatcherError::Empty);
        assert_eq!(
            err(MatcherKind::Domain, "github.com/org"),
            MatcherError::DomainWithPath
        );
        assert!(matches!(
            err(MatcherKind::Regex, "("),
            MatcherError::Regex(_)
        ));
    }

    #[test]
    fn wildcard_escapes_regex_syntax() {
        let m = matcher(MatcherKind::Wildcard, "example.com/a+b?c=*");
        assert!(hits(&m, "example.com/a+b?c=1"));
        assert!(!hits(&m, "example.com/aab?c=1"));
    }
}
