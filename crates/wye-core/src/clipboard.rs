//! The pure part of the clipboard features (EXT-02, EXT-03, EXT-05, EXT-12,
//! EXT-13, EXT-15, TRAY-10): what a clipboard change means and what to write
//! back. Watching the clipboard, the write and the Songlink request belong to
//! the service.
//!
//! [`decide`] takes what the clipboard offers and returns a [`Rewrite`]: leave
//! it, write new text, or ask Songlink first. Everything it needs to know
//! about the offer arrives in a [`ClipboardOffer`], so the decision is the
//! same on X11, Wayland and Klipper.

use url::Url;

use crate::clean::TrackingRules;
use crate::config::Extras;
use crate::host::host_matches;

mod songlink;

pub use songlink::{
    api_url as songlink_api_url, is_supported as songlink_supported, page_url as songlink_page,
};

/// The value of `x-kde-passwordManagerHint` that marks a secret (EXT-12).
pub const PASSWORD_HINT_SECRET: &str = "secret";

/// The clipboard rewrites that are switched on (EXT-02, EXT-03, EXT-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per switch on the Extras page"
)]
pub struct RewriteOptions {
    pub strip_tracking: bool,
    pub strip_mailto: bool,
    pub songlink: bool,
}

impl RewriteOptions {
    #[must_use]
    pub const fn from_extras(extras: &Extras) -> Self {
        Self {
            strip_tracking: extras.strip_tracking_on_copy,
            strip_mailto: extras.strip_mailto_on_copy,
            songlink: extras.songlink_on_copy,
        }
    }

    /// Whether the clipboard needs watching at all (EXT-12).
    #[must_use]
    pub const fn any(self) -> bool {
        self.strip_tracking || self.strip_mailto || self.songlink
    }
}

/// What the clipboard currently offers, as the provider found it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipboardOffer {
    /// The `text/plain` content, when there is some.
    pub text: Option<String>,
    /// The clipboard also offers content other than plain text: HTML,
    /// images, files (EXT-12: rich content is left alone).
    pub rich: bool,
    /// The value of the `x-kde-passwordManagerHint` format, when offered.
    pub password_manager_hint: Option<String>,
}

/// The text Wye last wrote, so its own write is not rewritten again
/// (EXT-12: "no loops").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwnWrites {
    last: Option<String>,
}

impl OwnWrites {
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// A guard that also knows `text` as Wye's own.
    #[must_use]
    pub fn remember(&self, text: &str) -> Self {
        Self {
            last: Some(text.to_owned()),
        }
    }

    #[must_use]
    pub fn is_own(&self, text: &str) -> bool {
        self.last.as_deref() == Some(text)
    }
}

/// What to do with a clipboard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rewrite {
    /// Leave the clipboard alone.
    Unchanged,
    /// Write this text back, once (EXT-12).
    Write(String),
    /// Convert the link with Songlink (EXT-15): ask `query` through
    /// [`songlink_api_url`] and write the page it names. When that fails the
    /// clipboard gets `fallback` (the link with its tracking removed), or
    /// stays as it is when there is none.
    Songlink {
        query: Url,
        fallback: Option<String>,
    },
}

/// Decides what a clipboard change means (EXT-12).
///
/// Nothing happens for rich content, content a password manager marked
/// secret, text Wye wrote itself, and anything that is not one line of text.
/// Then, in the order of 11-url-pipeline.md ("Clipboard pipeline"): tracking
/// removal (EXT-02), `mailto:` removal (EXT-03), music link conversion
/// (EXT-05).
#[must_use]
pub fn decide(
    offer: &ClipboardOffer,
    own: &OwnWrites,
    options: RewriteOptions,
    tracking: &TrackingRules,
) -> Rewrite {
    let Some(text) = offer.text.as_deref() else {
        return Rewrite::Unchanged;
    };
    if !options.any()
        || offer.rich
        || is_secret(offer.password_manager_hint.as_deref())
        || own.is_own(text)
    {
        return Rewrite::Unchanged;
    }
    let Some(line) = single_line(text) else {
        return Rewrite::Unchanged;
    };
    if let Some(address) = mailto_address(line) {
        return if options.strip_mailto {
            Rewrite::Write(address)
        } else {
            Rewrite::Unchanged
        };
    }
    match web_link(line) {
        Some(url) => rewrite_link(url, options, tracking),
        None => Rewrite::Unchanged,
    }
}

fn rewrite_link(mut url: Url, options: RewriteOptions, tracking: &TrackingRules) -> Rewrite {
    let cleaned = options.strip_tracking && !tracking.strip(&mut url).is_empty();
    let cleaned_text = cleaned.then(|| url.to_string());
    if options.songlink && songlink_supported(&url) {
        return Rewrite::Songlink {
            query: url,
            fallback: cleaned_text,
        };
    }
    cleaned_text.map_or(Rewrite::Unchanged, Rewrite::Write)
}

fn is_secret(hint: Option<&str>) -> bool {
    hint.is_some_and(|value| value.trim().eq_ignore_ascii_case(PASSWORD_HINT_SECRET))
}

/// The text without surrounding whitespace, when it is one non-empty line
/// (EXT-12).
#[must_use]
pub fn single_line(text: &str) -> Option<&str> {
    let line = text.trim();
    (!line.is_empty() && !line.contains(['\n', '\r'])).then_some(line)
}

fn web_link(line: &str) -> Option<Url> {
    if line.chars().any(char::is_whitespace) {
        return None;
    }
    Url::parse(line)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
}

/// The web link on the clipboard, when the text is one (TRAY-10, IN-02): one
/// line, no spaces, `http` or `https`. The menu's "Open URL from Clipboard"
/// is enabled exactly when this is `Some`.
#[must_use]
pub fn clipboard_link(text: &str) -> Option<Url> {
    single_line(text).and_then(web_link)
}

/// EXT-13: `mailto:name@example.com?subject=x` is `name@example.com`. The
/// query goes with the prefix. `None` when the text is not a `mailto:` link
/// with an address.
#[must_use]
pub fn mailto_address(line: &str) -> Option<String> {
    let prefix = line.get(..7)?;
    if !prefix.eq_ignore_ascii_case("mailto:") {
        return None;
    }
    let address = line.get(7..)?.split('?').next().unwrap_or_default().trim();
    let looks_like_address = address.contains('@')
        && !address.chars().any(char::is_whitespace)
        && !address.starts_with('@');
    looks_like_address.then(|| address.to_owned())
}

/// True when `host` is `domain` or one of its subdomains (shared with the
/// Songlink check).
fn on_domain(url: &Url, domain: &str) -> bool {
    url.host_str()
        .is_some_and(|host| host_matches(host, domain))
}

#[cfg(test)]
mod tests;
