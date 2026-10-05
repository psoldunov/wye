//! Sign-in pages (DEF-09).
//!
//! A desktop app that needs the user to sign in hands the login page to the
//! default browser, which is Wye. Sending that page back to the app the
//! service is mapped to would trap the user in a loop, and an OAuth flow
//! that ends on a loopback address only works in a browser. The pipeline
//! therefore keeps these pages away from app targets; this module only
//! recognises them.

use url::Url;

use crate::pipeline::is_web;

/// Host names whose first label marks a sign-in service, such as
/// `accounts.google.com`, `login.microsoftonline.com` or `auth.atlassian.com`.
const HOST_LABELS: [&str; 6] = ["accounts", "auth", "login", "oauth", "signin", "sso"];

/// Path segments that mark a sign-in or authorisation step. Whole segments
/// only, so `/authors/` and `/loginradius/` do not match.
const PATH_SEGMENTS: [&str; 20] = [
    "app_auth",
    "auth",
    "authorise",
    "authorize",
    "log-in",
    "login",
    "logon",
    "oauth",
    "oauth2",
    "oauth_authorize",
    "openid",
    "saml",
    "saml2",
    "sign-in",
    "sign-up",
    "sign_in",
    "sign_up",
    "signin",
    "signup",
    "sso",
];

/// How many path segments, counted from the start, may name a sign-in step.
const LEADING_SEGMENTS: usize = 2;

/// The fewest labels a host needs for its first label to name a sign-in
/// service: `auth.com` is a domain, `auth.example.com` is a service.
const MIN_HOST_LABELS: usize = 3;

/// True when `url` is a web page where the user signs in or authorises an
/// app (DEF-09): the host starts with a label such as `accounts` or
/// `login`, or one of the first two path segments is a sign-in word such as
/// `oauth`, `login` or `app_auth`. Only `http` and `https` links qualify.
///
/// Only the leading path segments count because the names users choose come
/// later: a Figma file named "Login" is `/design/<key>/Login` and a Linear
/// issue titled "Auth" is `/<workspace>/issue/AA-1/auth`. The trade-off is
/// accepted: a page whose own name sits in the first two segments, such as
/// the Telegram channel `t.me/login`, opens in a browser, where the service
/// offers to open its app.
#[must_use]
pub fn is_sign_in_page(url: &Url) -> bool {
    is_web(url) && (has_sign_in_host(url) || has_sign_in_path(url))
}

fn has_sign_in_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let mut labels = host.split('.');
    let first = labels.next().unwrap_or_default();
    labels.count() + 1 >= MIN_HOST_LABELS
        && HOST_LABELS
            .iter()
            .any(|label| first.eq_ignore_ascii_case(label))
}

fn has_sign_in_path(url: &Url) -> bool {
    url.path_segments().is_some_and(|segments| {
        segments
            .filter(|segment| !segment.is_empty())
            .take(LEADING_SEGMENTS)
            .any(|segment| {
                PATH_SEGMENTS
                    .iter()
                    .any(|word| segment.eq_ignore_ascii_case(word))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sign_in(link: &str) -> bool {
        is_sign_in_page(&Url::parse(link).unwrap())
    }

    #[test]
    fn recognises_sign_in_pages() {
        for link in [
            "https://www.figma.com/app_auth/6f4c/grant?desktop_protocol=figma",
            "https://www.figma.com/login",
            "https://www.figma.com/oauth?client_id=x",
            "https://linear.app/oauth/authorize?client_id=x",
            "https://linear.app/auth/desktop-redirect",
            "https://app.asana.com/-/oauth_authorize",
            "https://discord.com/api/oauth2/authorize",
            "https://api.notion.com/v1/oauth/authorize",
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://slack.com/signin",
            "https://LINEAR.app/LOGIN",
            "https://login.microsoftonline.com/common/oauth2/authorize",
            "https://auth.atlassian.com/",
            "http://example.com/sso/start",
        ] {
            assert!(is_sign_in(link), "{link}");
        }
    }

    #[test]
    fn leaves_ordinary_pages_alone() {
        for link in [
            "https://www.figma.com/design/abc/Login",
            "https://linear.app/almost-always/issue/AA-137/login",
            "https://linear.app/loginradius/issue/X-1",
            "https://example.com/authors/jane",
            "https://auth.com/",
            "https://www.figma.com/design/YmWx/Artian?node-id=1",
            "https://example.com/",
        ] {
            assert!(!is_sign_in(link), "{link}");
        }
    }

    #[test]
    fn only_web_links_qualify() {
        assert!(!is_sign_in("file:///home/me/login/index.html"));
        assert!(!is_sign_in("figma://login"));
    }
}
