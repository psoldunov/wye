//! The extension's messages and the host's replies (BEXT-01, BEXT-05,
//! BEXT-06).
//!
//! A link: `{"url": "…", "modifiers": ["Shift", …] | null, "pageOrLink":
//! "page" | "link"}`. `modifiers` are the keys held during the click, as the
//! browser reports them (Firefox's `OnClickData.modifiers`); `null` when the
//! browser does not report them (Chromium), so the service probes. A check
//! that the host is installed: `{"ping": true}`. Replies: `{"ok": true}` or
//! `{"error": "…"}`.

use serde::{Deserialize, Serialize};
use wye_api::context::Modifier;

/// Longest URL the host passes on.
const MAX_URL: usize = 32 * 1024;

/// One message from the extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Open `link` with Wye (IN-05).
    Open(Link),
    /// Is the host there (BEXT-06)?
    Ping,
}

/// A link from the extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub url: String,
    /// The held modifiers, or `None` when the browser does not say.
    pub held: Option<Vec<Modifier>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    #[serde(default)]
    ping: bool,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    modifiers: Option<Vec<String>>,
    #[serde(default)]
    page_or_link: Option<String>,
}

/// The host's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Reply {
    Ok { ok: bool },
    Error { error: String },
}

impl Reply {
    /// Done.
    #[must_use]
    pub const fn ok() -> Self {
        Self::Ok { ok: true }
    }

    /// Not done, and why, in words the extension shows.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            error: message.into(),
        }
    }

    /// The reply as JSON.
    #[must_use]
    pub fn to_json(&self) -> Vec<u8> {
        // Strings and booleans always encode.
        serde_json::to_vec(self).unwrap_or_else(|_| br#"{"error":"cannot encode"}"#.to_vec())
    }
}

/// Read one message.
///
/// # Errors
///
/// A message for the extension to show: not JSON, no URL, a URL that is too
/// long, or an unknown `pageOrLink`.
pub fn parse(body: &[u8]) -> Result<Request, String> {
    let wire: Wire =
        serde_json::from_slice(body).map_err(|error| format!("not a Wye message: {error}"))?;
    if wire.ping {
        return Ok(Request::Ping);
    }
    let url = wire
        .url
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
        .ok_or("the message has no URL")?;
    if url.len() > MAX_URL {
        return Err(format!("the URL is longer than {MAX_URL} bytes"));
    }
    // A page and a link on it route the same way; the value is only checked.
    match wire.page_or_link.as_deref() {
        None | Some("link" | "page") => {}
        Some(other) => return Err(format!("unknown pageOrLink `{other}`")),
    }
    Ok(Request::Open(Link {
        url,
        held: wire.modifiers.map(|names| held(&names)),
    }))
}

/// The modifiers Wye knows among the browser's names; left and right count
/// the same, and `Command` is the Super key on Linux.
fn held(names: &[String]) -> Vec<Modifier> {
    let known: Vec<Modifier> = names
        .iter()
        .filter_map(|name| match name.as_str() {
            "Shift" => Some(Modifier::Shift),
            "Ctrl" | "MacCtrl" => Some(Modifier::Ctrl),
            "Alt" => Some(Modifier::Alt),
            "Command" | "Super" | "Meta" => Some(Modifier::Super),
            _ => None,
        })
        .collect();
    Modifier::ALL
        .iter()
        .copied()
        .filter(|modifier| known.contains(modifier))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_with_held_keys_bext_05() {
        let request =
            parse(br#"{"url":" https://a.example/ ","modifiers":["Ctrl","Shift","Ctrl"],"pageOrLink":"link"}"#)
                .expect("a link");
        assert_eq!(
            request,
            Request::Open(Link {
                url: "https://a.example/".into(),
                held: Some(vec![Modifier::Shift, Modifier::Ctrl]),
            })
        );
    }

    #[test]
    fn unknown_keys_are_dropped_and_command_is_super() {
        let Request::Open(link) =
            parse(br#"{"url":"https://a.example/","modifiers":["Command","Fn"]}"#).expect("link")
        else {
            panic!("not a link");
        };
        assert_eq!(link.held, Some(vec![Modifier::Super]));
    }

    #[test]
    fn a_page_without_modifiers_leaves_them_to_the_service() {
        let Request::Open(link) =
            parse(br#"{"url":"https://a.example/","pageOrLink":"page","modifiers":null}"#)
                .expect("page")
        else {
            panic!("not a link");
        };
        assert_eq!(link.held, None);
    }

    #[test]
    fn a_ping_checks_the_host_bext_06() {
        assert_eq!(parse(br#"{"ping":true}"#), Ok(Request::Ping));
    }

    #[test]
    fn bad_messages_say_why() {
        assert!(
            parse(b"nope")
                .expect_err("not JSON")
                .contains("not a Wye message")
        );
        assert_eq!(
            parse(br#"{"url":"  "}"#),
            Err("the message has no URL".into())
        );
        assert!(parse(br#"{"url":"https://a/","pageOrLink":"tab"}"#).is_err());
        let long = format!(r#"{{"url":"https://a/{}"}}"#, "x".repeat(MAX_URL));
        assert!(parse(long.as_bytes()).is_err());
    }

    #[test]
    fn replies_are_ok_or_error() {
        assert_eq!(Reply::ok().to_json(), br#"{"ok":true}"#);
        assert_eq!(Reply::error("no").to_json(), br#"{"error":"no"}"#);
    }
}
