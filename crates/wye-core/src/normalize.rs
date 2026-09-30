//! The normalised link that rules match against (RUL-15).
//!
//! Wye removes the scheme and a leading `www.` before matching, so users
//! write `github.com/org` rather than `https://www.github.com/org`. Host
//! comparison ignores case; the path, query and fragment keep theirs.

use url::Url;

/// A link prepared for matching: `host[:port]/path[?query][#fragment]`,
/// lowercase host, no scheme, no leading `www.`, no user info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchUrl {
    text: String,
    host_len: usize,
}

impl MatchUrl {
    #[must_use]
    pub fn new(url: &Url) -> Self {
        let host = url.host_str().unwrap_or_default();
        let host = strip_www(host);
        let mut text = String::with_capacity(url.as_str().len());
        text.push_str(host);
        if let Some(port) = url.port() {
            text.push(':');
            text.push_str(&port.to_string());
        }
        let host_len = text.len();
        text.push_str(url.path());
        if let Some(query) = url.query() {
            text.push('?');
            text.push_str(query);
        }
        if let Some(fragment) = url.fragment() {
            text.push('#');
            text.push_str(fragment);
        }
        Self { text, host_len }
    }

    /// The whole normalised link.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The host without port, lowercase, without `www.`.
    #[must_use]
    pub fn host(&self) -> &str {
        let host_and_port = self.text.get(..self.host_len).unwrap_or(&self.text);
        // IPv6 literals keep their brackets, so a colon inside them is not a
        // port separator.
        match host_and_port.rsplit_once(':') {
            Some((host, port)) if !port.contains(']') => host,
            _ => host_and_port,
        }
    }
}

/// Normalises a user-written pattern the same way as links: drops a scheme,
/// a leading `www.` and lowercases the host part (everything before the first
/// `/`).
#[must_use]
pub fn normalize_pattern(pattern: &str) -> String {
    let pattern = pattern.trim();
    let without_scheme = pattern
        .split_once("://")
        .filter(|(scheme, _)| is_scheme(scheme))
        .map_or(pattern, |(_, rest)| rest);
    let (host, rest) = without_scheme
        .find(['/', '?', '#'])
        .map_or((without_scheme, ""), |i| without_scheme.split_at(i));
    let host = host.to_ascii_lowercase();
    format!("{}{rest}", strip_www(&host))
}

fn is_scheme(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn strip_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(link: &str) -> MatchUrl {
        MatchUrl::new(&Url::parse(link).unwrap())
    }

    #[test]
    fn strips_scheme_and_www() {
        let m = norm("https://www.GitHub.com/Org/Repo?x=1#Top");
        assert_eq!(m.as_str(), "github.com/Org/Repo?x=1#Top");
        assert_eq!(m.host(), "github.com");
    }

    #[test]
    fn keeps_explicit_port_and_drops_userinfo() {
        let m = norm("http://user:pw@localhost:8080/a");
        assert_eq!(m.as_str(), "localhost:8080/a");
        assert_eq!(m.host(), "localhost");
    }

    #[test]
    fn ipv6_host() {
        let m = norm("http://[::1]:3000/");
        assert_eq!(m.host(), "[::1]");
        assert_eq!(norm("http://[::1]/").host(), "[::1]");
    }

    #[test]
    fn pattern_normalisation() {
        assert_eq!(
            normalize_pattern("https://www.GitHub.com/Org"),
            "github.com/Org"
        );
        assert_eq!(
            normalize_pattern("Docs.Google.com/spreadsheets"),
            "docs.google.com/spreadsheets"
        );
        assert_eq!(normalize_pattern("/pull/"), "/pull/");
        assert_eq!(
            normalize_pattern("*.Atlassian.net/browse/*"),
            "*.atlassian.net/browse/*"
        );
    }
}
