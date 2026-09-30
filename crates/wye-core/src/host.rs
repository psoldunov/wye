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

#[cfg(test)]
mod tests {
    use super::host_matches;

    #[test]
    fn domain_and_subdomains() {
        assert!(host_matches("youtube.com", "youtube.com"));
        assert!(host_matches("m.YouTube.com", "youtube.com"));
        assert!(host_matches("a.b.example.org.", "*.example.org"));
        assert!(!host_matches("notyoutube.com", "youtube.com"));
        assert!(!host_matches("com", "youtube.com"));
    }
}
