//! Shortening links for display (PICK-09, TRAY-15, DLG-HIS-02): the host is
//! kept apart so the interface can emphasise it, and long text is cut in the
//! middle so the host and the end of the path stay visible.

use url::Url;

/// The ellipsis used when text is cut.
pub const ELLIPSIS: char = '…';

/// A link split for display: the host first, then the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkParts {
    /// The host (without a leading `www.`, with a non-default port),
    /// emphasised in the interface.
    pub host: String,
    /// Path, query and fragment, dimmed in the interface. Empty for a bare
    /// host.
    pub rest: String,
}

impl LinkParts {
    /// Splits `url`. A `file` link has no host, so the whole path is the
    /// host part.
    #[must_use]
    pub fn new(url: &Url) -> Self {
        let Some(host) = url.host_str() else {
            return Self {
                host: url.path().to_owned(),
                rest: String::new(),
            };
        };
        let host = host.strip_prefix("www.").unwrap_or(host);
        // PICK-09, TRAY-15: a non-default port belongs to the host, or every
        // local server reads as the same "localhost".
        let host = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let mut rest = url.path().to_owned();
        if rest == "/" {
            rest.clear();
        }
        if let Some(query) = url.query() {
            rest.push('?');
            rest.push_str(query);
        }
        if let Some(fragment) = url.fragment() {
            rest.push('#');
            rest.push_str(fragment);
        }
        Self { host, rest }
    }

    /// `host` and `rest` together.
    #[must_use]
    pub fn text(&self) -> String {
        format!("{}{}", self.host, self.rest)
    }

    /// The same, at most `max` characters long. The host stays whole when it
    /// fits; the cut falls in the rest, keeping its start and its end.
    #[must_use]
    pub fn truncated(&self, max: usize) -> Self {
        let host_len = self.host.chars().count();
        if host_len + self.rest.chars().count() <= max {
            return self.clone();
        }
        if host_len >= max {
            return Self {
                host: middle_truncate(&self.host, max),
                rest: String::new(),
            };
        }
        Self {
            host: self.host.clone(),
            rest: middle_truncate(&self.rest, max - host_len),
        }
    }
}

/// Cuts `text` to at most `max` characters by replacing the middle with an
/// ellipsis. `max` of 0 gives the empty string.
#[must_use]
pub fn middle_truncate(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_owned();
    }
    match max {
        0 => String::new(),
        1 => ELLIPSIS.to_string(),
        _ => {
            let keep = max - 1;
            let head = keep.div_ceil(2);
            let tail = keep - head;
            let mut out: String = chars.iter().take(head).collect();
            out.push(ELLIPSIS);
            out.extend(chars.iter().skip(chars.len() - tail));
            out
        }
    }
}

/// The link as "host and path" for menus (TRAY-15), at most `max`
/// characters, cut in the middle. Text that is not a link is cut as it is.
#[must_use]
pub fn host_and_path(link: &str, max: usize) -> String {
    Url::parse(link).map_or_else(
        |_| middle_truncate(link, max),
        |url| LinkParts::new(&url).truncated(max).text(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(link: &str) -> LinkParts {
        LinkParts::new(&Url::parse(link).unwrap())
    }

    #[test]
    fn splits_host_from_the_rest() {
        let p = parts("https://www.github.com/example/repo/pull/42?tab=files#diff");
        assert_eq!(p.host, "github.com");
        assert_eq!(p.rest, "/example/repo/pull/42?tab=files#diff");
        assert_eq!(p.text(), "github.com/example/repo/pull/42?tab=files#diff");
    }

    #[test]
    fn a_bare_host_has_no_rest() {
        assert_eq!(parts("https://example.com/").rest, "");
        assert_eq!(parts("https://example.com").text(), "example.com");
    }

    #[test]
    fn a_port_stays_with_the_host() {
        // PICK-09, TRAY-15: two servers on one host differ only in the port.
        let p = parts("http://localhost:41977/");
        assert_eq!(p.host, "localhost:41977");
        assert_eq!(p.rest, "");
        assert_eq!(
            parts("https://www.example.com:8443/a?b").text(),
            "example.com:8443/a?b"
        );
        assert_eq!(parts("http://[::1]:8080/x").text(), "[::1]:8080/x");
        // A scheme's default port is not written.
        assert_eq!(parts("https://example.com:443/").text(), "example.com");
        assert_eq!(
            host_and_path("http://127.0.0.1:3000/docs", 40),
            "127.0.0.1:3000/docs"
        );
    }

    #[test]
    fn file_links_show_their_path() {
        let p = parts("file:///home/me/page.html");
        assert_eq!(p.host, "/home/me/page.html");
        assert_eq!(p.rest, "");
    }

    #[test]
    fn middle_truncation_keeps_both_ends() {
        assert_eq!(middle_truncate("abcdefghij", 10), "abcdefghij");
        assert_eq!(middle_truncate("abcdefghij", 7), "abc…hij");
        assert_eq!(middle_truncate("abcdefghij", 6), "abc…ij");
        assert_eq!(middle_truncate("abcdefghij", 2), "a…");
        assert_eq!(middle_truncate("abcdefghij", 1), "…");
        assert_eq!(middle_truncate("abcdefghij", 0), "");
        assert_eq!(middle_truncate("ж", 5), "ж");
        assert_eq!(middle_truncate("жжжжжжжж", 4), "жж…ж");
    }

    #[test]
    fn truncation_keeps_the_host_and_the_end_of_the_path() {
        let p = parts("https://github.com/example/repo/pull/42/files").truncated(28);
        assert_eq!(p.host, "github.com");
        assert!(p.rest.starts_with("/exam"));
        assert!(p.rest.ends_with("/files"));
        assert!(p.rest.contains(ELLIPSIS));
        assert_eq!(p.text().chars().count(), 28);
    }

    #[test]
    fn a_host_longer_than_the_budget_is_cut_itself() {
        let p = parts("https://a-very-long-subdomain.example.com/x").truncated(10);
        assert_eq!(p.rest, "");
        assert_eq!(p.host.chars().count(), 10);
        assert!(p.host.contains(ELLIPSIS));
    }

    #[test]
    fn short_links_are_not_cut() {
        let p = parts("https://example.com/a");
        assert_eq!(p.truncated(40), p);
    }

    #[test]
    fn host_and_path_for_menus() {
        assert_eq!(
            host_and_path("https://www.example.com/a/b", 40),
            "example.com/a/b"
        );
        assert_eq!(host_and_path("not a link at all, really", 9), "not …ally");
        let long = host_and_path("https://example.com/some/very/long/path/to/a/page", 24);
        assert_eq!(long.chars().count(), 24);
        assert!(long.starts_with("example.com"));
    }
}
