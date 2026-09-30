//! The web app catalogue behind the Apps page and the pipeline's built-in
//! rules (APP-07, PIPE-08).

use serde::Deserialize;
use url::Url;

use crate::host::host_matches;
use crate::target::DesktopId;

const SHIPPED: &str = include_str!("../../../data/services.toml");

/// How a link is handed to the service's own desktop app (LAUNCH-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HandOver {
    /// Pass the `https` link unchanged.
    #[default]
    PassThrough,
    /// `open.spotify.com/track/<id>` becomes `spotify:track:<id>`.
    SpotifyUri,
    /// `steam://openurl/<link>`.
    SteamOpenurl,
}

impl HandOver {
    /// The link to give the desktop app.
    #[must_use]
    pub fn apply(self, url: &Url) -> String {
        match self {
            Self::PassThrough => url.to_string(),
            Self::SpotifyUri => spotify_uri(url).unwrap_or_else(|| url.to_string()),
            Self::SteamOpenurl => format!("steam://openurl/{url}"),
        }
    }
}

fn spotify_uri(url: &Url) -> Option<String> {
    const KINDS: [&str; 6] = ["track", "album", "artist", "playlist", "episode", "show"];
    let mut segments = url.path_segments()?.filter(|s| !s.is_empty()).peekable();
    // Localised links carry a leading "intl-de" style segment.
    if segments.peek().is_some_and(|s| s.starts_with("intl-")) {
        segments.next();
    }
    let kind = segments.next().filter(|k| KINDS.contains(k))?;
    let id = segments
        .next()
        .filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric()))?;
    Some(format!("spotify:{kind}:{id}"))
}

/// One service in the catalogue.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ServiceDefinition {
    pub id: String,
    pub name: String,
    hosts: Vec<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    pub desktop_apps: Vec<DesktopId>,
    #[serde(default)]
    pub hand_over: HandOver,
}

impl ServiceDefinition {
    #[must_use]
    pub fn matches(&self, url: &Url) -> bool {
        let host = url.host_str().unwrap_or_default();
        self.hosts.iter().any(|h| host_matches(host, h))
            && (self.paths.is_empty()
                || self
                    .paths
                    .iter()
                    .any(|p| url.path().starts_with(p.as_str())))
    }

    /// True when `app` is this service's own desktop app.
    #[must_use]
    pub fn is_own_app(&self, app: &DesktopId) -> bool {
        self.desktop_apps.contains(app)
    }
}

/// The catalogue, sorted by service name (APP-03).
#[derive(Debug, Clone)]
pub struct ServiceCatalogue {
    services: Vec<ServiceDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueFile {
    service: Vec<ServiceDefinition>,
}

impl ServiceCatalogue {
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
        let file: CatalogueFile = toml::from_str(SHIPPED).expect("data/services.toml is valid");
        let mut services = file.service;
        services.sort_by_cached_key(|s| s.name.to_lowercase());
        Self { services }
    }

    #[must_use]
    pub fn services(&self) -> &[ServiceDefinition] {
        &self.services
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ServiceDefinition> {
        self.services.iter().find(|s| s.id == id)
    }

    /// Services whose URL patterns match the link, in catalogue order.
    pub fn matching<'a>(&'a self, url: &'a Url) -> impl Iterator<Item = &'a ServiceDefinition> {
        self.services.iter().filter(move |s| s.matches(url))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn url(link: &str) -> Url {
        Url::parse(link).unwrap()
    }

    #[test]
    fn shipped_catalogue_is_sorted_with_unique_ids() {
        let catalogue = ServiceCatalogue::shipped();
        let names: Vec<_> = catalogue
            .services()
            .iter()
            .map(|s| s.name.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        let ids: HashSet<_> = catalogue.services().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), catalogue.services().len());
        // The services visible in the design (06-apps.md).
        for id in [
            "airtable",
            "amazon-chime",
            "around",
            "asana",
            "claude",
            "clickup",
            "discord",
            "figma",
            "front",
            "google-meet",
            "jitsi-meet",
            "linear",
        ] {
            assert!(catalogue.get(id).is_some(), "{id}");
        }
    }

    #[test]
    fn matching_by_host() {
        let catalogue = ServiceCatalogue::shipped();
        let ids = |link: &str| {
            let u = url(link);
            catalogue
                .matching(&u)
                .map(|s| s.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("https://meet.google.com/abc-defg-hij"), ["google-meet"]);
        assert_eq!(ids("https://us02web.zoom.us/j/123"), ["zoom"]);
        assert_eq!(ids("https://discord.gg/invite"), ["discord"]);
        assert!(ids("https://google.com/").is_empty());
    }

    #[test]
    fn hand_over() {
        assert_eq!(
            HandOver::SpotifyUri.apply(&url(
                "https://open.spotify.com/intl-de/track/4uLU6hMCjMI75M1A2tKUQC?si=x"
            )),
            "spotify:track:4uLU6hMCjMI75M1A2tKUQC"
        );
        assert_eq!(
            HandOver::SpotifyUri.apply(&url("https://open.spotify.com/genre/x")),
            "https://open.spotify.com/genre/x"
        );
        assert_eq!(
            HandOver::SteamOpenurl.apply(&url("https://store.steampowered.com/app/1/")),
            "steam://openurl/https://store.steampowered.com/app/1/"
        );
        assert_eq!(
            HandOver::PassThrough.apply(&url("https://a.example/")),
            "https://a.example/"
        );
    }
}
