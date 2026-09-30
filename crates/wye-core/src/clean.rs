//! Link cleaning: tracking-parameter removal (EXT-01, EXT-10) and forced
//! HTTPS (EXT-04, EXT-14).

use std::net::IpAddr;

use serde::Deserialize;
use url::{Host, Url};

use crate::host::host_matches;

const SHIPPED: &str = include_str!("../../../data/tracking-parameters.toml");

/// The tracking-parameter list ([`data/tracking-parameters.toml`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackingRules {
    global: Vec<String>,
    prefixes: Vec<String>,
    #[serde(default, rename = "site")]
    sites: Vec<SiteRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteRule {
    /// Documents the entry in the data file.
    #[serde(rename = "name")]
    _name: String,
    hosts: Vec<String>,
    params: Vec<String>,
}

impl TrackingRules {
    /// The list that ships with Wye.
    ///
    /// # Panics
    ///
    /// Panics if the embedded data file is malformed, which the unit tests
    /// rule out.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "embedded data is validated by unit tests"
    )]
    pub fn shipped() -> Self {
        toml::from_str(SHIPPED).expect("data/tracking-parameters.toml is valid")
    }

    fn is_tracking(&self, host: &str, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        self.global.iter().any(|g| g.eq_ignore_ascii_case(&name))
            || self.prefixes.iter().any(|p| name.starts_with(p.as_str()))
            || self.sites.iter().any(|site| {
                site.hosts.iter().any(|h| host_matches(host, h))
                    && site.params.iter().any(|p| p.eq_ignore_ascii_case(&name))
            })
    }

    /// Removes tracking parameters and returns the names removed, in link
    /// order. Every other parameter keeps its exact spelling and position,
    /// and the fragment is kept.
    pub fn strip(&self, url: &mut Url) -> Vec<String> {
        let host = url.host_str().unwrap_or_default().to_owned();
        let Some(query) = url.query() else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        let kept: Vec<&str> = query
            .split('&')
            .filter(|pair| {
                if pair.is_empty() {
                    return false;
                }
                let raw_name = pair.split_once('=').map_or(*pair, |(name, _)| name);
                let name = decode_name(raw_name);
                if self.is_tracking(&host, &name) {
                    removed.push(name);
                    false
                } else {
                    true
                }
            })
            .collect();
        if removed.is_empty() {
            return removed;
        }
        let new_query = kept.join("&");
        url.set_query((!new_query.is_empty()).then_some(new_query.as_str()));
        removed
    }
}

fn decode_name(raw: &str) -> String {
    url::form_urlencoded::parse(raw.as_bytes())
        .next()
        .map_or_else(|| raw.to_owned(), |(name, _)| name.into_owned())
}

/// Rewrites `http` to `https` (EXT-14). Returns true when the link changed.
///
/// Left alone: `localhost` and `*.localhost`, `*.local`, single-label
/// intranet names, IP literals (which covers private ranges), and links with
/// an explicit port.
pub fn force_https(url: &mut Url) -> bool {
    if url.scheme() != "http" || url.port().is_some() {
        return false;
    }
    let exempt = match url.host() {
        None | Some(Host::Ipv4(_) | Host::Ipv6(_)) => true,
        Some(Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            let domain = domain.strip_suffix('.').unwrap_or(&domain);
            domain.parse::<IpAddr>().is_ok()
                || !domain.contains('.')
                || host_matches(domain, "localhost")
                || domain.rsplit('.').next() == Some("local")
        }
    };
    !exempt && url.set_scheme("https").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(link: &str) -> (String, Vec<String>) {
        let mut url = Url::parse(link).unwrap();
        let removed = TrackingRules::shipped().strip(&mut url);
        (url.to_string(), removed)
    }

    #[test]
    fn shipped_list_parses() {
        let rules = TrackingRules::shipped();
        assert!(rules.global.contains(&"fbclid".to_owned()));
    }

    #[test]
    fn help_text_example() {
        assert_eq!(
            strip("https://shop.example/item?id=7&utm_source=news").0,
            "https://shop.example/item?id=7"
        );
    }

    #[test]
    fn keeps_other_params_order_encoding_and_fragment() {
        let (link, removed) =
            strip("https://a.example/p?b=2&UTM_Medium=x&q=a%20b+c&fbclid=1&a=1#frag");
        assert_eq!(link, "https://a.example/p?b=2&q=a%20b+c&a=1#frag");
        assert_eq!(removed, ["UTM_Medium", "fbclid"]);
    }

    #[test]
    fn drops_empty_query() {
        assert_eq!(
            strip("https://a.example/?gclid=1&utm_x=2").0,
            "https://a.example/"
        );
    }

    #[test]
    fn site_specific_params() {
        assert_eq!(
            strip("https://youtu.be/abc?si=XYZ&t=10").0,
            "https://youtu.be/abc?t=10"
        );
        assert_eq!(
            strip("https://example.com/?si=keep").0,
            "https://example.com/?si=keep"
        );
    }

    #[test]
    fn untouched_link_is_byte_identical() {
        let link = "https://a.example/p?x=%2F&y";
        assert_eq!(strip(link), (link.to_owned(), vec![]));
    }

    #[test]
    fn https_upgrade_and_exceptions() {
        let upgrade = |link: &str| {
            let mut url = Url::parse(link).unwrap();
            force_https(&mut url).then(|| url.to_string())
        };
        assert_eq!(
            upgrade("http://example.com/a?b").as_deref(),
            Some("https://example.com/a?b")
        );
        for exempt in [
            "http://localhost/",
            "http://app.localhost/",
            "http://printer.local/",
            "http://router/",
            "http://192.168.1.1/",
            "http://[::1]/",
            "http://example.com:8080/",
            "https://example.com/",
        ] {
            assert_eq!(upgrade(exempt), None, "{exempt}");
        }
    }
}
