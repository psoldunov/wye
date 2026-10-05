//! The web app catalogue behind the Apps page and the pipeline's built-in
//! rules (APP-07, PIPE-08).

use serde::Deserialize;
use url::Url;

use crate::host::host_matches;
use crate::sign_in::is_sign_in_page;
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
    sign_in_paths: Vec<String>,
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

    /// True when `url` is a sign-in or authorisation page of this service
    /// (DEF-09): one the generic detector [`is_sign_in_page`] recognises, or
    /// one under a `sign-in-paths` prefix. The catalogue lists only the
    /// routes the generic words miss, such as `ClickUp`'s `/api`. A prefix
    /// matches at a path segment boundary, ignoring ASCII case: `/api`
    /// matches `/api` and `/api/x` but not `/apis`; a prefix that ends in
    /// `/` is a plain prefix.
    #[must_use]
    pub fn is_sign_in(&self, url: &Url) -> bool {
        is_sign_in_page(url)
            || self
                .sign_in_paths
                .iter()
                .any(|prefix| has_path_prefix(url.path(), prefix))
    }

    /// True when `app` is this service's own desktop app.
    #[must_use]
    pub fn is_own_app(&self, app: &DesktopId) -> bool {
        self.desktop_apps.contains(app)
    }

    /// True when an installed app called `app_name` is this service's own
    /// desktop app by name (APP-12). The comparison ignores case and runs of
    /// spaces; a bracketed qualifier at the end of the service name may be
    /// left out, and one trailing "for Linux", "Desktop" or "Linux" on the
    /// app's name is ignored. A service with a translated hand-over never
    /// matches: only the apps the catalogue lists accept its links.
    #[must_use]
    pub fn owns_app_named(&self, app_name: &str) -> bool {
        if self.hand_over != HandOver::PassThrough {
            return false;
        }
        let service = normalise_name(&self.name);
        let service_candidates = [
            Some(service.as_str()),
            strip_bracketed_suffix(&service).filter(|short| !short.is_empty()),
        ];
        let app = normalise_name(app_name);
        let app_candidates = [Some(app.as_str()), strip_platform_suffix(&app)];
        app_candidates
            .into_iter()
            .flatten()
            .filter(|candidate| !candidate.is_empty())
            .any(|candidate| {
                service_candidates
                    .into_iter()
                    .flatten()
                    .any(|s| s == candidate)
            })
    }
}

/// Trimmed, lower-cased, with runs of whitespace collapsed to one space.
fn normalise_name(name: &str) -> String {
    name.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// `"element (matrix)"` becomes `"element"`; `None` without a trailing
/// bracketed group.
fn strip_bracketed_suffix(name: &str) -> Option<&str> {
    let body = name.strip_suffix(')')?;
    let open = body.rfind('(')?;
    name.get(..open).map(str::trim)
}

/// The name without one trailing whole-word platform suffix (APP-12); the
/// suffix needs a leading space, so "Desktop" alone stays as it is.
fn strip_platform_suffix(name: &str) -> Option<&str> {
    const SUFFIXES: [&str; 3] = [" for linux", " desktop", " linux"];
    SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .map(str::trim)
}

/// True when `path` starts with `prefix`, ignoring ASCII case, and the match
/// ends at a segment boundary unless `prefix` itself ends in `/`.
fn has_path_prefix(path: &str, prefix: &str) -> bool {
    let Some(head) = path.get(..prefix.len()) else {
        return false;
    };
    head.eq_ignore_ascii_case(prefix)
        && (prefix.ends_with('/')
            || path.len() == prefix.len()
            || path.as_bytes().get(prefix.len()) == Some(&b'/'))
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

    fn owns(service: &str, app_name: &str) -> bool {
        ServiceCatalogue::shipped()
            .get(service)
            .unwrap()
            .owns_app_named(app_name)
    }

    // APP-12
    #[test]
    fn app12_an_app_named_after_the_service_is_its_own_app() {
        for name in [
            "Claude",
            "claude",
            "  Claude ",
            "Claude Desktop",
            "Claude for Linux",
            "Claude Linux",
        ] {
            assert!(owns("claude", name), "{name:?}");
        }
        for name in ["Claude Code", "Clauder", "Desktop", ""] {
            assert!(!owns("claude", name), "{name:?}");
        }
    }

    // APP-12
    #[test]
    fn app12_a_bracketed_qualifier_of_the_service_name_may_be_left_out() {
        for name in ["Element", "Element (Matrix)", "Element Desktop"] {
            assert!(owns("element", name), "{name:?}");
        }
        assert!(!owns("linear", "Linear Algebra"));
    }

    // APP-12
    #[test]
    fn app12_a_translated_hand_over_finds_no_app_by_name() {
        assert!(!owns("spotify", "Spotify"));
        assert!(!owns("steam", "Steam"));
    }

    fn service_with_sign_in_paths(paths: &[&str]) -> ServiceDefinition {
        let listed: Vec<String> = paths.iter().map(|p| format!("{p:?}")).collect();
        toml::from_str(&format!(
            "id = \"x\"\nname = \"X\"\nhosts = [\"x.test\"]\nsign-in-paths = [{}]\n",
            listed.join(", ")
        ))
        .unwrap()
    }

    // DEF-09
    #[test]
    fn sign_in_paths_match_at_a_segment_boundary_ignoring_case() {
        let service = service_with_sign_in_paths(&["/api", "/connect/"]);
        for link in [
            "https://x.test/api",
            "https://x.test/api/x",
            "https://x.test/API?client_id=x",
            "https://x.test/Api/",
            "https://x.test/connect/google",
            "https://x.test/login",
        ] {
            assert!(service.is_sign_in(&url(link)), "{link}");
        }
        for link in [
            "https://x.test/apis",
            "https://x.test/apiary/x",
            "https://x.test/v1/api",
            "https://x.test/connect",
            "https://x.test/",
        ] {
            assert!(!service.is_sign_in(&url(link)), "{link}");
        }
    }

    #[test]
    fn a_service_without_sign_in_paths_knows_only_the_generic_words() {
        let service = service_with_sign_in_paths(&[]);
        assert!(service.is_sign_in(&url("https://x.test/oauth/authorize")));
        assert!(!service.is_sign_in(&url("https://x.test/api")));
    }

    // DEF-09: each shipped service recognises its own sign-in pages.
    #[test]
    fn every_service_knows_its_sign_in_pages() {
        const PAGES: &[(&str, &str)] = &[
            (
                "figma",
                "https://www.figma.com/app_auth/6f4c/grant?desktop_protocol=figma",
            ),
            ("figma", "https://www.figma.com/login"),
            ("figma", "https://www.figma.com/oauth?client_id=x"),
            ("linear", "https://linear.app/login"),
            ("linear", "https://linear.app/oauth/authorize?client_id=x"),
            ("linear", "https://linear.app/auth/desktop-redirect"),
            ("slack", "https://slack.com/signin"),
            ("slack", "https://slack.com/oauth/v2/authorize?client_id=x"),
            ("slack", "https://slack.com/openid/connect/authorize"),
            ("slack", "https://acme.slack.com/sso/saml/start"),
            ("discord", "https://discord.com/login"),
            (
                "discord",
                "https://discord.com/oauth2/authorize?client_id=x",
            ),
            ("discord", "https://discord.com/api/oauth2/authorize"),
            ("zoom", "https://zoom.us/signin"),
            ("zoom", "https://zoom.us/oauth/authorize?client_id=x"),
            ("zoom", "https://acme.zoom.us/saml/login"),
            ("notion", "https://www.notion.so/login"),
            ("notion", "https://api.notion.com/v1/oauth/authorize"),
            ("asana", "https://app.asana.com/-/login"),
            ("asana", "https://app.asana.com/-/oauth_authorize"),
            ("airtable", "https://airtable.com/login"),
            ("airtable", "https://airtable.com/oauth2/v1/authorize"),
            ("claude", "https://claude.ai/login"),
            ("claude", "https://claude.ai/oauth/authorize?client_id=x"),
            ("clickup", "https://app.clickup.com/login"),
            (
                "clickup",
                "https://app.clickup.com/api?client_id=x&redirect_uri=y",
            ),
            ("front", "https://app.frontapp.com/signin"),
            ("front", "https://app.frontapp.com/oauth/authorize"),
            ("steam", "https://store.steampowered.com/login/"),
            ("steam", "https://steamcommunity.com/openid/login"),
            ("steam", "https://steamcommunity.com/login/home/"),
            ("webex", "https://signin.webex.com/collabs/auth"),
            (
                "webex",
                "https://idbroker.webex.com/idb/oauth2/v1/authorize",
            ),
            ("zulip", "https://zulipchat.com/login/"),
            ("zulip", "https://acme.zulipchat.com/accounts/login/"),
        ];
        let catalogue = ServiceCatalogue::shipped();
        for (id, link) in PAGES {
            let service = catalogue.get(id).unwrap_or_else(|| panic!("{id}"));
            let link = url(link);
            assert!(service.matches(&link), "{id} does not match {link}");
            assert!(service.is_sign_in(&link), "{id}: {link}");
        }
    }

    // DEF-09: pages with content stay with the service's app.
    #[test]
    fn content_links_are_not_sign_in_pages() {
        let catalogue = ServiceCatalogue::shipped();
        for link in [
            "https://www.figma.com/design/YmWx/Artian?node-id=1",
            "https://www.figma.com/design/abc/Login",
            "https://linear.app/almost-always/issue/AA-137/login",
            "https://app.slack.com/client/T1/C2",
            "https://discord.com/channels/1/2",
            "https://discord.gg/abcdef",
            "https://us02web.zoom.us/j/123",
            "https://www.notion.so/acme/Login-0123456789abcdef",
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            "https://store.steampowered.com/app/1/",
            "https://meet.google.com/abc-defg-hij",
            "https://app.clickup.com/9012/v/l/2",
        ] {
            let link = url(link);
            let services: Vec<_> = catalogue.matching(&link).collect();
            assert!(!services.is_empty(), "no service matches {link}");
            for service in services {
                assert!(!service.is_sign_in(&link), "{}: {link}", service.id);
            }
        }
    }
}
