//! URL expansion (PIPE-03, ADV-01): unwrapping redirect wrappers and
//! recognising short links.
//!
//! Redirect wrappers are unwrapped here, locally. Short links need a network
//! request. This crate owns the policy of the redirect chain (DLG-EXP-03)
//! and recognises the short links; a [`ShortLinkResolver`] sends the
//! requests, one hop at a time.

use serde::Deserialize;
use url::Url;

use crate::config::ExpansionSettings;
use crate::hooks::{ResolveError, ShortLinkResolver};
use crate::host::host_matches;

const SHIPPED: &str = include_str!("../../../data/expansion.toml");

/// The shipped list of wrappers and short-link domains
/// ([`data/expansion.toml`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ExpansionCatalogue {
    short_links: Vec<String>,
    #[serde(rename = "wrapper")]
    wrappers: Vec<Wrapper>,
}

/// A redirect wrapper such as `google.com/url?q=…`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wrapper {
    pub id: String,
    pub name: String,
    hosts: Vec<String>,
    paths: Vec<String>,
    params: Vec<String>,
}

impl Wrapper {
    /// The host patterns the wrapper applies to, as in `data/expansion.toml`
    /// (DLG-EXP-01).
    #[must_use]
    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    fn applies_to(&self, url: &Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        let path = url.path().trim_end_matches('/');
        self.hosts
            .iter()
            .any(|pattern| wrapper_host_matches(host, pattern))
            && self.paths.iter().any(|p| p.trim_end_matches('/') == path)
    }

    fn target(&self, url: &Url) -> Option<Url> {
        self.params.iter().find_map(|param| {
            url.query_pairs()
                .find(|(name, _)| name == param.as_str())
                .and_then(|(_, value)| Url::parse(&value).ok())
                .filter(|target| matches!(target.scheme(), "http" | "https"))
        })
    }
}

fn wrapper_host_matches(host: &str, pattern: &str) -> bool {
    match pattern.strip_suffix(".*") {
        // "google.*": google.<TLD>, optionally behind www. The TLD part has
        // at most two labels (`com`, `co.uk`), so `google.com.evil.example`
        // is not Google.
        Some(label) => {
            let host = host.to_ascii_lowercase();
            let host = host.strip_prefix("www.").unwrap_or(&host);
            host.strip_prefix(label)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|tld| {
                    tld.split('.').count() <= 2
                        && tld
                            .split('.')
                            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphabetic()))
                })
        }
        None => host_matches(host, pattern),
    }
}

/// One unwrapping step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unwrapped {
    pub wrapper: String,
    pub url: Url,
}

impl ExpansionCatalogue {
    /// The catalogue that ships with Wye.
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
        toml::from_str(SHIPPED).expect("data/expansion.toml is valid")
    }

    #[must_use]
    pub fn wrappers(&self) -> &[Wrapper] {
        &self.wrappers
    }

    #[must_use]
    pub fn short_links(&self) -> &[String] {
        &self.short_links
    }

    /// Unwraps nested redirect wrappers, at most `max_redirects` times, using
    /// only the wrappers the settings leave enabled.
    #[must_use]
    pub fn unwrap(&self, url: &Url, settings: &ExpansionSettings) -> Vec<Unwrapped> {
        let mut steps: Vec<Unwrapped> = Vec::new();
        let mut current = url.clone();
        while steps.len() < usize::from(settings.max_redirects) {
            let next = self
                .wrappers
                .iter()
                .filter(|w| settings.is_enabled(&w.id))
                .find(|w| w.applies_to(&current))
                .and_then(|w| w.target(&current).map(|target| (w.id.clone(), target)));
            let Some((wrapper, target)) = next else { break };
            current = target.clone();
            steps.push(Unwrapped {
                wrapper,
                url: target,
            });
        }
        steps
    }

    /// True when the link's host is an enabled short-link domain (shipped or
    /// added by the user) that network expansion would resolve.
    #[must_use]
    pub fn is_short_link(&self, url: &Url, settings: &ExpansionSettings) -> bool {
        let host = url.host_str().unwrap_or_default();
        self.short_links
            .iter()
            .chain(&settings.custom_short_links)
            .filter(|domain| settings.is_enabled(domain))
            .any(|domain| host_matches(host, domain))
    }
}

/// Why the redirect chain of a short link ended (DLG-EXP-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandStop {
    /// The link is not an enabled short link, so nothing was contacted.
    NotShortLink,
    /// The chain reached its destination: the last link answered without
    /// redirecting, or it is not an enabled short-link domain and was not
    /// contacted (DLG-EXP-03).
    Destination,
    /// The chain used up `max_redirects` hops; the last link may redirect
    /// further.
    LimitReached,
    /// The next redirect leaves the web (`intent:`, `spotify:` …); the chain
    /// stops at the last web link.
    NonWeb(String),
    /// The `Location` could not be read as a link.
    BadLocation(String),
    /// A redirect led back to a link already visited.
    Loop,
    /// A request failed or timed out (PIPE-03: the pipeline continues with
    /// the link reached so far).
    Failed(ResolveError),
}

impl ExpandStop {
    /// What to tell the user when the link could not be expanded fully
    /// (DLG-EXP-04), or `None` when the chain ended normally.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        match self {
            Self::NotShortLink | Self::Destination => None,
            Self::LimitReached => Some("too many redirects".to_owned()),
            Self::NonWeb(scheme) => Some(format!("it redirects to a {scheme}: link")),
            Self::BadLocation(location) => Some(format!(
                "it redirects to something that is not a link ({location:?})"
            )),
            Self::Loop => Some("it redirects in a circle".to_owned()),
            Self::Failed(error) => Some(error.to_string()),
        }
    }
}

/// The result of following a short link over the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    /// Every link reached, in order, without the starting link.
    pub hops: Vec<Url>,
    pub stop: ExpandStop,
}

impl Expanded {
    /// The link the chain ended on, or `None` when it never left the
    /// starting link.
    #[must_use]
    pub fn last(&self) -> Option<&Url> {
        self.hops.last()
    }
}

impl ExpansionCatalogue {
    /// Follows a short link's redirects (PIPE-03, DLG-EXP-03).
    ///
    /// Every link the resolver is asked about must be an enabled short-link
    /// domain (DLG-EXP-03). The chain follows up to `settings.max_redirects`
    /// redirects, each `Location` resolved against the link that sent it (it
    /// may be relative). A `Location` on any other host is the end: it is
    /// returned as the last hop without a request to it. It also stops at a
    /// non-web `Location`, a loop, or a failed request.
    pub fn expand_short_link(
        &self,
        start: &Url,
        settings: &ExpansionSettings,
        resolver: &dyn ShortLinkResolver,
    ) -> Expanded {
        let mut hops: Vec<Url> = Vec::new();
        if !self.is_short_link(start, settings) {
            return Expanded {
                hops,
                stop: ExpandStop::NotShortLink,
            };
        }
        let mut current = start.clone();
        let stop = loop {
            if hops.len() >= usize::from(settings.max_redirects) {
                break ExpandStop::LimitReached;
            }
            let location = match resolver.resolve(&current) {
                Ok(Some(location)) => location,
                Ok(None) => break ExpandStop::Destination,
                Err(error) => break ExpandStop::Failed(error),
            };
            let Ok(next) = current.join(location.as_str().trim()) else {
                break ExpandStop::BadLocation(location.as_str().to_owned());
            };
            if !matches!(next.scheme(), "http" | "https") {
                break ExpandStop::NonWeb(next.scheme().to_owned());
            }
            if next.host().is_none() {
                break ExpandStop::BadLocation(location.as_str().to_owned());
            }
            if next == *start || hops.contains(&next) {
                break ExpandStop::Loop;
            }
            hops.push(next.clone());
            // DLG-EXP-03: only enabled short-link domains are contacted. The
            // `Location` came from the previous answer, so a host that is
            // not enabled ends the chain here, without a request to it.
            if !self.is_short_link(&next, settings) {
                break ExpandStop::Destination;
            }
            current = next;
        };
        Expanded { hops, stop }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unwrap_all(link: &str, settings: &ExpansionSettings) -> Vec<(String, String)> {
        ExpansionCatalogue::shipped()
            .unwrap(&Url::parse(link).unwrap(), settings)
            .into_iter()
            .map(|u| (u.wrapper, u.url.to_string()))
            .collect()
    }

    #[test]
    fn shipped_catalogue_parses() {
        let catalogue = ExpansionCatalogue::shipped();
        assert_eq!(catalogue.wrappers().len(), 6);
        assert!(catalogue.short_links().iter().any(|d| d == "bit.ly"));
    }

    #[test]
    fn unwraps_every_shipped_wrapper() {
        let target = "https%3A%2F%2Fexample.com%2Fa%3Fb%3D1";
        let settings = ExpansionSettings::default();
        for (link, wrapper) in [
            (
                format!("https://www.google.co.uk/url?sa=t&q={target}"),
                "google",
            ),
            (
                format!("https://l.facebook.com/l.php?u={target}&h=AT0"),
                "facebook",
            ),
            (format!("https://l.instagram.com/?u={target}"), "facebook"),
            (
                format!("https://eur01.safelinks.protection.outlook.com/?url={target}&data=x"),
                "safelinks",
            ),
            (
                format!("https://slack-redir.net/link?url={target}"),
                "slack",
            ),
            (
                format!("https://steamcommunity.com/linkfilter/?u={target}"),
                "steam",
            ),
            (
                format!("https://www.youtube.com/redirect?event=x&q={target}"),
                "youtube",
            ),
        ] {
            assert_eq!(
                unwrap_all(&link, &settings),
                [(wrapper.to_owned(), "https://example.com/a?b=1".to_owned())],
                "{link}"
            );
        }
    }

    #[test]
    fn nested_wrappers_and_redirect_limit() {
        let inner = "https://slack-redir.net/link?url=https%3A%2F%2Fexample.com%2F";
        let outer = format!(
            "https://www.google.com/url?q={}",
            url::form_urlencoded::byte_serialize(inner.as_bytes()).collect::<String>()
        );
        assert_eq!(unwrap_all(&outer, &ExpansionSettings::default()).len(), 2);
        let limited = ExpansionSettings {
            max_redirects: 1,
            ..ExpansionSettings::default()
        };
        assert_eq!(unwrap_all(&outer, &limited).len(), 1);
    }

    #[test]
    fn disabled_wrapper_is_skipped() {
        let settings = ExpansionSettings {
            disabled: vec!["google".into()],
            ..ExpansionSettings::default()
        };
        assert!(unwrap_all("https://google.com/url?q=https://example.com/", &settings).is_empty());
    }

    #[test]
    fn non_web_targets_and_lookalikes_are_ignored() {
        let settings = ExpansionSettings::default();
        assert!(unwrap_all("https://google.com/url?q=javascript:alert(1)", &settings).is_empty());
        assert!(
            unwrap_all(
                "https://notgoogle.com/url?q=https://example.com/",
                &settings
            )
            .is_empty()
        );
        assert!(
            unwrap_all(
                "https://google.com/search?q=https://example.com/",
                &settings
            )
            .is_empty()
        );
    }

    #[test]
    fn google_lookalike_with_more_labels_is_not_a_wrapper() {
        let settings = ExpansionSettings::default();
        let link = |host: &str| format!("https://{host}/url?q=https://example.com/");
        assert!(unwrap_all(&link("google.com.evil.example"), &settings).is_empty());
        assert!(unwrap_all(&link("www.google.co.uk.evil"), &settings).is_empty());
        assert_eq!(unwrap_all(&link("google.co.uk"), &settings).len(), 1);
        assert_eq!(unwrap_all(&link("www.google.de"), &settings).len(), 1);
    }

    #[test]
    fn short_links() {
        let catalogue = ExpansionCatalogue::shipped();
        let settings = ExpansionSettings {
            disabled: vec!["t.co".into()],
            custom_short_links: vec!["go.example".into()],
            ..ExpansionSettings::default()
        };
        let is_short = |link: &str| catalogue.is_short_link(&Url::parse(link).unwrap(), &settings);
        assert!(is_short("https://bit.ly/abc"));
        assert!(is_short("https://go.example/x"));
        assert!(!is_short("https://t.co/abc"));
        assert!(!is_short("https://example.com/"));
    }
}

#[cfg(test)]
mod chain_tests;
