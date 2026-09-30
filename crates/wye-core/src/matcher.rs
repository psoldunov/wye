//! URL matchers (RUL-14): the five ways a rule can test a link.
//!
//! Every matcher works on the normalised link ([`MatchUrl`]). The examples in
//! `docs/spec/19-help-texts.md` ("URL matchers") are the tests below.

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::normalize::{
    MatchUrl, normalize_contains, normalize_host, normalize_pattern, split_host, split_port,
    strip_scheme,
};

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
    /// The normalised link contains the pattern. The pattern may span the
    /// host and the path, and it is matched case-sensitively after removing
    /// the scheme and `www.` from both; the host of a link is lowercase, so
    /// write host parts of the pattern in lowercase.
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
    #[error(
        "a domain matcher takes a host name such as github.com, without a port; use \"Starts with\" to match a port"
    )]
    DomainWithPort,
    #[error("a domain matcher takes a host name, without user info (`@`)")]
    DomainWithUserInfo,
    #[error("a domain matcher needs a host name; `*` alone would match every link")]
    DomainWildcardOnly,
    #[error("invalid regular expression: {0}")]
    Regex(String),
}

impl UrlMatcher {
    /// Validates and prepares the matcher.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty pattern, a domain with a path, a port,
    /// user info or nothing but a wildcard, or an invalid regular expression.
    pub fn compile(&self) -> Result<CompiledMatcher, MatcherError> {
        let raw = self.pattern.trim();
        if raw.is_empty() {
            return Err(MatcherError::Empty);
        }
        let compiled = match self.kind {
            MatcherKind::Domain => CompiledMatcher::Domain(compile_domain(raw)?),
            MatcherKind::Prefix => CompiledMatcher::Prefix(normalize_pattern(raw)),
            // "Contains" often holds a path fragment such as `/pull/`, so it
            // keeps its case; a host part must be written in lowercase.
            MatcherKind::Contains => CompiledMatcher::Contains(normalize_contains(raw)),
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

/// A domain pattern: a host, optionally behind a scheme, a leading `*.`
/// (same meaning as the bare domain) or followed by a single `/`.
fn compile_domain(raw: &str) -> Result<String, MatcherError> {
    let rest = strip_scheme(raw);
    let stripped = rest.strip_prefix("*.");
    let (host, tail) = split_host(stripped.unwrap_or(rest));
    if host.contains('@') {
        return Err(MatcherError::DomainWithUserInfo);
    }
    if host == "*" || (stripped.is_some() && host.is_empty()) {
        return Err(MatcherError::DomainWildcardOnly);
    }
    if host.is_empty() {
        return Err(MatcherError::Empty);
    }
    if !matches!(tail, "" | "/") || host.chars().any(char::is_whitespace) {
        return Err(MatcherError::DomainWithPath);
    }
    if split_port(host).1.is_some() {
        return Err(MatcherError::DomainWithPort);
    }
    Ok(normalize_host(host))
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
    fn domain_rejects_user_info_and_a_bare_wildcard() {
        let err = |pattern: &str| {
            UrlMatcher {
                kind: MatcherKind::Domain,
                pattern: pattern.into(),
            }
            .compile()
            .unwrap_err()
        };
        assert_eq!(err("user@github.com"), MatcherError::DomainWithUserInfo);
        assert_eq!(
            err("https://u:p@github.com/"),
            MatcherError::DomainWithUserInfo
        );
        assert_eq!(err("*"), MatcherError::DomainWildcardOnly);
        assert_eq!(err("*.*"), MatcherError::DomainWildcardOnly);
        assert_eq!(err("*."), MatcherError::DomainWildcardOnly);
    }

    /// Review round 2: (kind, pattern, links that match, links that do not).
    #[test]
    fn normalisation_edge_cases() {
        let cases: [(_, _, &[&str], &[&str]); 7] = [
            (
                MatcherKind::Prefix,
                "a.example/x{1}",
                &["a.example/x{1}", "a.example/x{1}/more"],
                &["a.example/x{2}"],
            ),
            (
                MatcherKind::Prefix,
                "a.example/?q=it's",
                &["a.example/?q=it's"],
                &["a.example/?q=its"],
            ),
            (
                MatcherKind::Prefix,
                "github.com",
                &["github.com/x", "github.com:8443/x"],
                &[],
            ),
            (
                MatcherKind::Wildcard,
                "a.example/*{1}",
                &["a.example/x/{1}"],
                &[],
            ),
            (
                MatcherKind::Wildcard,
                "*README.md",
                &["github.com/x/README.md"],
                &["github.com/x/readme.md"],
            ),
            (
                MatcherKind::Wildcard,
                "*.Atlassian.net/browse/*",
                &["team.atlassian.net/browse/ABC-1"],
                &[],
            ),
            // The needle may span host and path; case is kept as written.
            (
                MatcherKind::Contains,
                "github.com/Org",
                &["GitHub.com/Org/x"],
                &["github.com/org/x"],
            ),
        ];
        for (kind, pattern, yes, no) in cases {
            let m = matcher(kind, pattern);
            for link in yes {
                assert!(hits(&m, link), "{kind:?} {pattern} should match {link}");
            }
            for link in no {
                assert!(
                    !hits(&m, link),
                    "{kind:?} {pattern} should not match {link}"
                );
            }
        }
        let upper = matcher(MatcherKind::Contains, "GitHub.com/Org");
        assert!(!hits(&upper, "github.com/Org/x"));
    }

    #[test]
    fn internationalised_patterns_match_like_links() {
        let domain = matcher(MatcherKind::Domain, "bücher.de");
        assert!(hits(&domain, "bücher.de/x"));
        assert!(hits(&domain, "shop.BÜCHER.de/x"));
        let accented = matcher(MatcherKind::Domain, "ÉXAMPLE.fr");
        assert!(hits(&accented, "éxample.fr/"));
        assert!(!hits(&accented, "example.fr/"));
        let prefix = matcher(MatcherKind::Prefix, "de.wikipedia.org/wiki/Zürich");
        assert!(hits(&prefix, "de.wikipedia.org/wiki/Zürich_(Stadt)"));
        let contains = matcher(MatcherKind::Contains, "/wiki/Zürich");
        assert!(hits(&contains, "de.wikipedia.org/wiki/Zürich"));
    }

    #[test]
    fn domain_accepts_a_leading_wildcard_and_rejects_a_port() {
        let m = matcher(MatcherKind::Domain, "*.github.com");
        assert!(hits(&m, "github.com/x"));
        assert!(hits(&m, "gist.github.com/x"));
        let err = UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: "localhost:8080".into(),
        }
        .compile()
        .unwrap_err();
        assert_eq!(err, MatcherError::DomainWithPort);
    }

    #[test]
    fn domain_matches_a_trailing_dot_host() {
        let m = matcher(MatcherKind::Domain, "github.com");
        assert!(hits(&m, "github.com./x"));
    }

    #[test]
    fn wildcard_without_a_host_keeps_its_case() {
        let m = matcher(MatcherKind::Wildcard, "*ABC-*");
        assert!(hits(&m, "jira.example/browse/ABC-1"));
        assert!(!hits(&m, "jira.example/browse/abc-1"));
    }

    #[test]
    fn contains_drops_scheme_and_www_but_keeps_case() {
        let m = matcher(MatcherKind::Contains, "https://www.github.com/Org");
        assert!(hits(&m, "www.github.com/Org/repo"));
        assert!(!hits(&m, "github.com/org/repo"));
    }

    #[test]
    fn wildcard_escapes_regex_syntax() {
        let m = matcher(MatcherKind::Wildcard, "example.com/a+b?c=*");
        assert!(hits(&m, "example.com/a+b?c=1"));
        assert!(!hits(&m, "example.com/aab?c=1"));
    }
}
