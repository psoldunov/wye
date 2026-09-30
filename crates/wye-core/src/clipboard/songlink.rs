//! Songlink (EXT-05, EXT-15): which links qualify and how to ask. The
//! endpoint and its terms are marked "verify" in the spec; they are
//! constants here so they change in one place.

use url::Url;

use super::on_domain;

/// The Odesli API behind song.link (EXT-15; verify the endpoint and terms).
pub const API_ENDPOINT: &str = "https://api.song.link/v1-alpha.1/links";

/// Hosts a returned `pageUrl` may have. Anything else is not trusted as a
/// Songlink page.
const PAGE_DOMAINS: [&str; 4] = ["song.link", "album.link", "artist.link", "odesli.co"];

/// A music service whose links Songlink converts: its hosts, the path
/// segments that may precede the kind, and the kinds of link it supports.
struct Service {
    domains: &'static [&'static str],
    skip: fn(&str) -> bool,
    kinds: &'static [&'static str],
}

fn is_country(segment: &str) -> bool {
    segment.len() == 2 && segment.bytes().all(|b| b.is_ascii_alphabetic())
}

const SERVICES: &[Service] = &[
    // Apple Music: /{country}/album/{name}/{id}, /{country}/song/{name}/{id}.
    Service {
        domains: &["music.apple.com", "itunes.apple.com"],
        skip: is_country,
        kinds: &["album", "song"],
    },
    // Spotify: /track/{id}, /album/{id}, optionally behind /intl-xx/.
    Service {
        domains: &["open.spotify.com"],
        skip: |segment| segment.starts_with("intl-"),
        kinds: &["track", "album"],
    },
    // TIDAL: /track/{id}, /album/{id}, optionally behind /browse/.
    Service {
        domains: &["tidal.com", "listen.tidal.com"],
        skip: |segment| segment == "browse",
        kinds: &["track", "album"],
    },
    // Deezer: /{language}/track/{id}, /{language}/album/{id}.
    Service {
        domains: &["deezer.com"],
        skip: is_country,
        kinds: &["track", "album"],
    },
];

/// True when Songlink can convert this link: a track or album on Apple
/// Music, Spotify, TIDAL or Deezer (EXT-05). Artists, playlists, users and
/// other pages are left alone, as are links that already are Songlink pages.
#[must_use]
pub fn is_supported(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    SERVICES
        .iter()
        .filter(|service| service.domains.iter().any(|d| on_domain(url, d)))
        .any(|service| has_supported_path(url, service))
}

fn has_supported_path(url: &Url, service: &Service) -> bool {
    let Some(segments) = url.path_segments() else {
        return false;
    };
    let mut segments = segments.filter(|s| !s.is_empty()).peekable();
    if segments.peek().is_some_and(|s| (service.skip)(s)) {
        segments.next();
    }
    let kind = segments.next();
    let id = segments.next();
    kind.is_some_and(|kind| service.kinds.contains(&kind)) && id.is_some()
}

/// The request for the Songlink page of `link` (EXT-15).
#[must_use]
pub fn api_url(link: &Url) -> Url {
    // The endpoint is a constant that parses; the fallback keeps this total.
    let mut url = Url::parse(API_ENDPOINT).unwrap_or_else(|_| link.clone());
    url.query_pairs_mut().append_pair("url", link.as_str());
    url
}

/// The Songlink page named by the API's answer (`pageUrl`), when the answer
/// is JSON with an `https` page on a Songlink domain (EXT-15). `None` for
/// anything else, so the clipboard stays unchanged.
#[must_use]
pub fn page_url(response: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(response).ok()?;
    let page = Url::parse(value.get("pageUrl")?.as_str()?).ok()?;
    let trusted =
        page.scheme() == "https" && PAGE_DOMAINS.iter().any(|domain| on_domain(&page, domain));
    trusted.then(|| page.to_string())
}
