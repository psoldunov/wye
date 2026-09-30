//! URL expansion (PIPE-03, ADV-01): unwrapping redirect wrappers and
//! recognising short links.
//!
//! Redirect wrappers are unwrapped here, locally. Short links need a network
//! request; this crate only recognises them, and the caller decides whether
//! and how to resolve them.

use serde::Deserialize;
use url::Url;

use crate::config::ExpansionSettings;
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
        // "google.*": google.<TLD>, optionally behind www.
        Some(label) => {
            let host = host.to_ascii_lowercase();
            let host = host.strip_prefix("www.").unwrap_or(&host);
            host.strip_prefix(label)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|tld| {
                    !tld.is_empty()
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
