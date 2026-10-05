//! Host-name patterns shared by the shipped data files.

/// True when `host` is `domain` or one of its subdomains. Both sides are
/// compared case-insensitively; a leading `*.` on `domain` is accepted and
/// means the same thing.
#[must_use]
pub fn host_matches(host: &str, domain: &str) -> bool {
    let domain = domain.strip_prefix("*.").unwrap_or(domain);
    let host = host.strip_suffix('.').unwrap_or(host);
    if host.len() < domain.len() {
        return false;
    }
    let (head, tail) = host.split_at(host.len() - domain.len());
    tail.eq_ignore_ascii_case(domain) && (head.is_empty() || head.ends_with('.'))
}

/// True when `host` is `name` itself, not one of its subdomains. Compared
/// case-insensitively; a trailing `.` on `host` is ignored.
#[must_use]
pub fn host_is(host: &str, name: &str) -> bool {
    host.strip_suffix('.')
        .unwrap_or(host)
        .eq_ignore_ascii_case(name)
}

#[cfg(test)]
mod tests {
    use super::{host_is, host_matches};

    #[test]
    fn domain_and_subdomains() {
        assert!(host_matches("youtube.com", "youtube.com"));
        assert!(host_matches("m.YouTube.com", "youtube.com"));
        assert!(host_matches("a.b.example.org.", "*.example.org"));
        assert!(!host_matches("notyoutube.com", "youtube.com"));
        assert!(!host_matches("com", "youtube.com"));
    }

    #[test]
    fn exact_host() {
        assert!(host_is("notion.so", "notion.so"));
        assert!(host_is("WWW.Notion.so.", "www.notion.so"));
        assert!(!host_is("mail.notion.so", "notion.so"));
        assert!(!host_is("notion.so", "www.notion.so"));
    }
}
