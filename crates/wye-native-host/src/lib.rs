//! `wye-native-host`: the browser extension's native-messaging host
//! (BEXT-04 to BEXT-06, IN-05).
//!
//! The browser starts it for the extension, as the manifest in
//! `wye_desktop::native_messaging` says, and talks to it over stdin and
//! stdout ([`framing`]). Each link becomes one `OpenLink` call with
//! `entry = "extension"`: the service forces the picker unless the bypass
//! key is held (ADV-10, ADV-11), with the keys held during the click
//! (BEXT-05) and the browser, the host's parent, as the source app. The call
//! starts the service through D-Bus activation when it is not running.

pub mod framing;
pub mod install;
pub mod message;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::time::Duration;

use wye_api::context::{self as keys, Entry};
use wye_api::proxy::Wye1Proxy;
use wye_core::SourceApp;
use zbus::zvariant::Value;

use self::message::{Link, Reply, Request};

/// Longest one link may take, starting the service included.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Answer every message on `reader` until the browser closes it, handing
/// links to `open`.
///
/// # Errors
///
/// A broken stream: a message cut short or over the size limit, or a reply
/// that cannot be written.
pub fn serve(
    reader: &mut impl Read,
    writer: &mut impl Write,
    open: &mut impl FnMut(&Link) -> Result<(), String>,
) -> io::Result<()> {
    while let Some(body) = framing::read_message(reader)? {
        let reply = match message::parse(&body) {
            Ok(Request::Ping) => Reply::ok(),
            Ok(Request::Open(link)) => open(&link).map_or_else(Reply::error, |()| Reply::ok()),
            Err(error) => Reply::error(error),
        };
        framing::write_message(writer, &reply.to_json())?;
    }
    Ok(())
}

/// The `OpenLink` context of `link` from the browser `source`, whose
/// process is `parent` (IN-05, BEXT-05).
#[must_use]
pub fn context(
    link: &Link,
    source: &SourceApp,
    parent: u32,
) -> HashMap<&'static str, Value<'static>> {
    let texts = [
        (keys::ENTRY, Some(Entry::Extension.as_str().to_owned())),
        (
            keys::SOURCE_DESKTOP_ID,
            source.desktop_id.as_ref().map(|id| id.as_str().to_owned()),
        ),
        (keys::SOURCE_EXECUTABLE, source.executable.clone()),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| (key, Value::from(value))));
    let held = link.held.as_ref().map(|held| {
        let names: Vec<String> = held.iter().map(|key| key.as_str().to_owned()).collect();
        [
            (keys::HELD, Value::from(names)),
            (keys::HELD_KNOWN, Value::from(true)),
        ]
    });
    let pid = source
        .desktop_id
        .is_none()
        .then(|| (keys::SOURCE_PID, Value::from(parent)));
    texts.chain(held.into_iter().flatten()).chain(pid).collect()
}

/// The service, over the session bus.
pub struct Service {
    runtime: tokio::runtime::Runtime,
    wye: Option<Wye1Proxy<'static>>,
}

impl Service {
    /// A runtime for the calls; the bus is connected on the first link.
    ///
    /// # Errors
    ///
    /// When the runtime cannot start.
    pub fn new() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(Self { runtime, wye: None })
    }

    /// Hand `link` to the service with `context`.
    ///
    /// # Errors
    ///
    /// Why Wye did not take the link, in words the extension shows.
    pub fn open(
        &mut self,
        link: &Link,
        context: HashMap<&'static str, Value<'static>>,
    ) -> Result<(), String> {
        let wye = if let Some(wye) = &self.wye {
            wye.clone()
        } else {
            let wye = self
                .runtime
                .block_on(connect())
                .map_err(|error| format!("Wye cannot be reached on the session bus: {error}"))?;
            self.wye = Some(wye.clone());
            wye
        };
        let call = async move { wye.open_link(&link.url, context).await };
        match self.runtime.block_on(tokio::time::timeout(TIMEOUT, call)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(format!("Wye did not open the link: {error}")),
            Err(_) => Err(format!("Wye did not answer within {TIMEOUT:?}")),
        }
    }
}

async fn connect() -> zbus::Result<Wye1Proxy<'static>> {
    let connection = zbus::Connection::session().await?;
    Wye1Proxy::new(&connection).await
}

#[cfg(test)]
mod tests {
    use std::thread;

    use wye_api::context::Modifier;
    use wye_core::DesktopId;

    use super::*;

    fn link(held: Option<Vec<Modifier>>) -> Link {
        Link {
            url: "https://a.example/".into(),
            held,
        }
    }

    fn text(context: &HashMap<&str, Value<'_>>, key: &str) -> Option<String> {
        context
            .get(key)
            .and_then(|value| String::try_from(value.try_clone().ok()?).ok())
    }

    #[test]
    fn the_browser_is_the_source_and_the_entry_is_the_extension_in_05() {
        let source = SourceApp {
            desktop_id: Some(DesktopId::new("firefox.desktop").expect("id")),
            executable: Some("firefox".into()),
        };
        let context = context(&link(None), &source, 42);
        assert_eq!(text(&context, keys::ENTRY).as_deref(), Some("extension"));
        assert_eq!(
            text(&context, keys::SOURCE_DESKTOP_ID).as_deref(),
            Some("firefox.desktop")
        );
        assert!(!context.contains_key(keys::SOURCE_PID));
        assert!(
            !context.contains_key(keys::HELD_KNOWN),
            "the service probes"
        );
    }

    #[test]
    fn keys_from_the_click_travel_as_known_bext_05() {
        let context = context(&link(Some(vec![Modifier::Alt])), &SourceApp::default(), 42);
        let held: Vec<String> = context
            .get(keys::HELD)
            .and_then(|value| Vec::<String>::try_from(value.try_clone().ok()?).ok())
            .expect("held");
        assert_eq!(held, ["Alt"]);
        assert_eq!(
            context
                .get(keys::HELD_KNOWN)
                .and_then(|v| bool::try_from(v).ok()),
            Some(true)
        );
        assert_eq!(
            context
                .get(keys::SOURCE_PID)
                .and_then(|v| u32::try_from(v).ok()),
            Some(42),
            "an unknown browser is detected by the service"
        );
    }

    #[test]
    fn serve_answers_every_message_over_pipes_bext_04() {
        let (mut from_browser, mut to_host) = std::io::pipe().expect("pipe");
        let (mut from_host, mut to_browser) = std::io::pipe().expect("pipe");
        let browser = thread::spawn(move || {
            framing::write_message(&mut to_host, br#"{"ping":true}"#).expect("ping");
            framing::write_message(&mut to_host, br#"{"url":"https://a.example/"}"#).expect("link");
            framing::write_message(&mut to_host, br#"{"url":"https://refused.example/"}"#)
                .expect("refused");
            framing::write_message(&mut to_host, b"nonsense").expect("nonsense");
        });
        let mut opened = Vec::new();
        serve(&mut from_browser, &mut to_browser, &mut |link: &Link| {
            opened.push(link.url.clone());
            if link.url.contains("refused") {
                Err("no".to_owned())
            } else {
                Ok(())
            }
        })
        .expect("served");
        browser.join().expect("browser");
        drop(to_browser);
        let replies: Vec<String> = std::iter::from_fn(|| {
            framing::read_message(&mut from_host)
                .expect("reply")
                .map(|body| String::from_utf8(body).expect("UTF-8"))
        })
        .collect();
        assert_eq!(opened, ["https://a.example/", "https://refused.example/"]);
        assert_eq!(replies[0], r#"{"ok":true}"#);
        assert_eq!(replies[1], r#"{"ok":true}"#);
        assert_eq!(replies[2], r#"{"error":"no"}"#);
        assert!(replies[3].starts_with(r#"{"error":"not a Wye message"#));
        assert_eq!(replies.len(), 4);
    }
}
