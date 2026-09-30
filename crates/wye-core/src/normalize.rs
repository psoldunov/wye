//! The normalised link that rules match against (RUL-15).
//!
//! Wye removes the scheme and a leading `www.` before matching, so users
//! write `github.com/org` rather than `https://www.github.com/org`. Host
//! comparison ignores case; the path, query and fragment keep theirs.
//! Patterns are normalised the way the `url` crate normalises links, so an
//! internationalised host, or a path or query written with characters the
//! crate encodes (`{`, `}`, `'`, non-ASCII), matches the link.

use url::Url;

/// A link prepared for matching: `host[:port]/path[?query][#fragment]`,
/// lowercase ASCII host without a trailing dot, no scheme, no leading
/// `www.`, no user info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchUrl {
    text: String,
    host_len: usize,
}

impl MatchUrl {
    #[must_use]
    pub fn new(url: &Url) -> Self {
        let host = url.host_str().unwrap_or_default();
        // `github.com.` is the same host as `github.com`.
        let host = host.strip_suffix('.').unwrap_or(host);
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

    /// The path, query and fragment, as they follow the host and port.
    fn tail(&self) -> &str {
        self.text.get(self.host_len..).unwrap_or_default()
    }

    /// The host without port, lowercase, without `www.`.
    #[must_use]
    pub fn host(&self) -> &str {
        let host_and_port = self.text.get(..self.host_len).unwrap_or(&self.text);
        split_port(host_and_port).0
    }
}

/// Normalises a user-written pattern the same way as links: drops a scheme;
/// when the pattern starts with a host, converts that host to lowercase
/// ASCII (punycode) and drops a leading `www.`; the path, query and
/// fragment go through the `url` crate like a link's, so they encode the
/// same. A pattern that does not start with a host, such as `/pull/` or
/// `*ABC-*`, keeps its case.
#[must_use]
pub fn normalize_pattern(pattern: &str) -> String {
    let rest = strip_scheme(pattern.trim());
    let (host, tail) = split_host(rest);
    if starts_with_host(rest, host, tail) {
        format!("{}{}", normalize_host(host), normalize_tail(tail))
    } else {
        encode(rest)
    }
}

/// Normalises a "Contains" pattern: drops a scheme and a leading `www.`,
/// which links never carry (RUL-11), and percent-encodes like the `url`
/// crate for the characters that every part of a link encodes.
///
/// The needle may span the host and the path (`github.com/Org`), and it is
/// matched case-sensitively against the normalised link, whose host is
/// lowercase. Write the host part of a needle in lowercase; the path keeps
/// its case, as in a link.
#[must_use]
pub fn normalize_contains(pattern: &str) -> String {
    encode(strip_www(strip_scheme(pattern.trim())))
}

/// Splits a pattern after its scheme into the host part (up to the first
/// `/`, `?` or `#`) and the rest.
pub(crate) fn split_host(pattern: &str) -> (&str, &str) {
    pattern
        .find(['/', '?', '#'])
        .map_or((pattern, ""), |i| pattern.split_at(i))
}

/// Splits `host[:port]` into the host and the port, if any. IPv6 literals
/// keep their brackets, so a colon inside them is not a port separator.
pub(crate) fn split_port(host_and_port: &str) -> (&str, Option<&str>) {
    match host_and_port.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => (host, Some(port)),
        _ => (host_and_port, None),
    }
}

/// Drops a leading `scheme://`.
pub(crate) fn strip_scheme(pattern: &str) -> &str {
    pattern
        .split_once("://")
        .filter(|(scheme, _)| is_scheme(scheme))
        .map_or(pattern, |(_, rest)| rest)
}

/// Converts a host pattern (optionally with a port and `*` wildcards) the
/// way the `url` crate converts a link's host: IDNA to ASCII, lowercase, no
/// trailing dot, no leading `www.`.
pub(crate) fn normalize_host(host: &str) -> String {
    let ascii = if host.contains('*') {
        // Wildcards cannot go through the URL parser; convert the labels
        // without one on their own (IDNA works label by label).
        host.split('.')
            .map(|label| {
                if label.contains('*') || label.is_ascii() {
                    label.to_ascii_lowercase()
                } else {
                    parsed_host(label).unwrap_or_else(|| label.to_lowercase())
                }
            })
            .collect::<Vec<_>>()
            .join(".")
    } else {
        parsed_host(host).unwrap_or_else(|| host.to_lowercase())
    };
    let (name, port) = split_port(&ascii);
    let name = strip_www(name.strip_suffix('.').unwrap_or(name));
    match port {
        Some(port) => format!("{name}:{port}"),
        None => name.to_owned(),
    }
}

/// The host (and non-default port) the `url` crate makes of `host`.
fn parsed_host(host: &str) -> Option<String> {
    let url = Url::parse(&format!("https://{host}/")).ok()?;
    let name = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{name}:{port}"),
        None => name.to_owned(),
    })
}

/// A pattern starts with a host when its first segment is followed by a
/// path, contains a dot or is an IPv6 literal. A first segment that starts
/// with `*` in a pattern without a `/` is a name fragment such as
/// `*README.md`, not a host.
fn starts_with_host(pattern: &str, host: &str, tail: &str) -> bool {
    let wildcard_fragment = host.starts_with('*') && !pattern.contains('/');
    !host.is_empty()
        && !wildcard_fragment
        && (tail.starts_with('/') || host.contains('.') || host.starts_with('['))
}

/// Encodes the path, query and fragment of a pattern by making a link of
/// them and normalising it, so `{`, `}` and `'` encode exactly as in the
/// links they are matched against. `*` is kept by the parser in each part.
fn normalize_tail(tail: &str) -> String {
    if tail.is_empty() {
        return String::new();
    }
    Url::parse(&format!("https://placeholder.invalid{tail}")).map_or_else(
        |_| encode(tail),
        |url| MatchUrl::new(&url).tail().to_owned(),
    )
}

/// Percent-encodes what the `url` crate encodes in every part of a link:
/// non-ASCII characters, controls, space, `"`, `<` and `>`.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_graphic() && !matches!(c, '"' | '<' | '>') {
            encoded.push(c);
        } else {
            let mut buffer = [0; 4];
            for byte in c.encode_utf8(&mut buffer).bytes() {
                encoded.push('%');
                for nibble in [byte >> 4, byte & 0xF] {
                    let digit = char::from_digit(u32::from(nibble), 16);
                    encoded.extend(digit.map(|d| d.to_ascii_uppercase()));
                }
            }
        }
    }
    encoded
}

fn is_scheme(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Drops a leading `www.`, unless that would leave a single label
/// (`www.com` stays as it is).
fn strip_www(host: &str) -> &str {
    host.strip_prefix("www.")
        .filter(|rest| rest.contains('.'))
        .unwrap_or(host)
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
    fn keeps_www_when_it_is_the_whole_name() {
        assert_eq!(norm("https://www.com/a").host(), "www.com");
        assert_eq!(normalize_pattern("www.com/a"), "www.com/a");
        assert_eq!(normalize_contains("www.com"), "www.com");
        assert_eq!(normalize_contains("www.github.com/x"), "github.com/x");
    }

    #[test]
    fn drops_a_trailing_dot_from_the_host() {
        let m = norm("https://github.com./x");
        assert_eq!(m.as_str(), "github.com/x");
        assert_eq!(m.host(), "github.com");
        assert_eq!(normalize_pattern("github.com./x"), "github.com/x");
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
        assert_eq!(normalize_pattern("*ABC-*"), "*ABC-*");
        assert_eq!(normalize_pattern("LocalHost:8080/x"), "localhost:8080/x");
    }

    #[test]
    fn a_wildcard_name_fragment_is_not_a_host() {
        assert_eq!(normalize_pattern("*README.md"), "*README.md");
        assert_eq!(normalize_pattern("*.Atlassian.net"), "*.Atlassian.net");
        assert_eq!(
            normalize_pattern("*.Atlassian.net/Browse/*"),
            "*.atlassian.net/Browse/*"
        );
    }

    #[test]
    fn patterns_encode_like_links() {
        for link in [
            "https://a.example/x{1}",
            "https://a.example/?q=it's",
            "https://a.example/a`b?c=d#e{f}",
            "https://a.example/x y?z=1 2",
        ] {
            assert_eq!(normalize_pattern(link), norm(link).as_str(), "{link}");
        }
        assert_eq!(normalize_pattern("a.example/x{1}"), "a.example/x%7B1%7D");
        assert_eq!(normalize_pattern("a.example/*{1}"), "a.example/*%7B1%7D");
    }

    #[test]
    fn a_pattern_without_a_path_gets_no_slash() {
        assert_eq!(normalize_pattern("github.com"), "github.com");
        assert_eq!(normalize_pattern("localhost:8080"), "localhost:8080");
    }

    #[test]
    fn patterns_convert_like_links() {
        let link = norm("https://bücher.de/wiki/Zürich");
        assert_eq!(normalize_pattern("Bücher.de/wiki/Zürich"), link.as_str());
        assert_eq!(normalize_pattern("*.bücher.de/*"), "*.xn--bcher-kva.de/*");
        assert_eq!(normalize_contains("/wiki/Zürich"), "/wiki/Z%C3%BCrich");
    }
}
