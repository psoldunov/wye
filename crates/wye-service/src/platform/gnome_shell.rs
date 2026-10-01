//! The GNOME Shell extension as the session helper
//! (`dev.soldunov.wye.SessionHelper1`, `docs/dbus-api.md`).
//!
//! Mutter offers no data-control protocol, no layer shell and no way to
//! ask for the pointer or the focused window, so on GNOME the service asks
//! the Shell, through Wye's extension: the clipboard (IN-02 to IN-04,
//! TRAY-10, EXT-12 to EXT-15), held modifiers (BRW-03, RUL-27, KEY-06),
//! the pointer (PICK-02) and the focused app (source-app step 4).
//!
//! The helper follows the extension's bus name: everything is unavailable
//! while the extension is not running or does not serve the interface, and
//! available again as soon as it does. The extension answers only the
//! owner of `dev.soldunov.wye`, and sends clipboard changes to it alone,
//! only while the service asked for them ([`ClipboardProvider::set_watching`]:
//! a copy-time rewrite is on).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use futures_lite::StreamExt as _;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use wye_api::context::Modifier;
use wye_api::names::{GNOME_BUS_NAME, GNOME_OBJECT_PATH, SESSION_HELPER_INTERFACE};
use wye_api::picker::Placement;
use wye_api::proxy::SessionHelper1Proxy;
use zbus::fdo::{DBusProxy, IntrospectableProxy};
use zbus::names::BusName;

use super::clipboard::single_line_text;
use super::{
    ClipboardCapabilities, ClipboardProvider, FocusSource, FocusedApp, ModifierSource,
    PlatformError, PointerSource,
};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "gnome-shell";

/// How long a query of the pointer, the modifiers or the focus may take,
/// as long as the other session probes allow.
pub const QUERY_TIMEOUT: Duration = Duration::from_millis(150);

/// How long a clipboard read or write may take.
pub const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(1);

/// Changes the watchers may fall behind by.
const BUFFER: usize = 16;

/// GNOME Shell's own bus name: owned in a GNOME Shell session and nowhere
/// else, unlike `XDG_CURRENT_DESKTOP`, which Budgie and GNOME Flashback
/// also set to include `GNOME`.
pub const SHELL_BUS_NAME: &str = "org.gnome.Shell";

/// How long asking the bus whether GNOME Shell runs may take.
const SESSION_CHECK_TIMEOUT: Duration = Duration::from_secs(1);

/// Whether this is a GNOME Shell session (ADV-12): `org.gnome.Shell` has an
/// owner on `connection`. One `NameHasOwner` call, asked each time, so a
/// Shell that starts after the service counts as soon as it runs. A bus that
/// cannot be asked counts as no GNOME session.
pub async fn is_gnome_session(connection: &zbus::Connection) -> bool {
    let asked = tokio::time::timeout(SESSION_CHECK_TIMEOUT, async {
        let name = BusName::try_from(SHELL_BUS_NAME)?;
        let owned = DBusProxy::new(connection)
            .await?
            .name_has_owner(name)
            .await?;
        Ok::<_, zbus::Error>(owned)
    })
    .await;
    match asked {
        Ok(Ok(owned)) => owned,
        Ok(Err(error)) => {
            tracing::debug!(%error, "cannot tell whether GNOME Shell runs");
            false
        }
        Err(_) => {
            tracing::debug!("the bus did not tell in time whether GNOME Shell runs");
            false
        }
    }
}

/// The extension's `SessionHelper1`, while it is there.
#[derive(Debug)]
pub struct ShellHelper {
    inner: Arc<Inner>,
    follower: JoinHandle<()>,
}

#[derive(Debug)]
struct Inner {
    connection: zbus::Connection,
    /// The helper at its owner's unique name; `None` while there is none.
    helper: Mutex<Option<SessionHelper1Proxy<'static>>>,
    /// Whether the service wants clipboard changes.
    watching: AtomicBool,
    changes: broadcast::Sender<String>,
}

impl ShellHelper {
    /// Follow the extension on `connection`: it may start before or after
    /// the service, restart with the Shell, or never come.
    #[must_use]
    pub fn start(connection: &zbus::Connection) -> Self {
        let inner = Arc::new(Inner {
            connection: connection.clone(),
            helper: Mutex::new(None),
            watching: AtomicBool::new(false),
            changes: broadcast::Sender::new(BUFFER),
        });
        let follower = tokio::spawn(follow(Arc::clone(&inner)));
        Self { inner, follower }
    }

    /// Whether the extension serves the helper now.
    #[must_use]
    pub fn present(&self) -> bool {
        self.inner.current().is_some()
    }

    fn mechanism(&self) -> Option<&'static str> {
        self.present().then_some(MECHANISM)
    }
}

impl Drop for ShellHelper {
    fn drop(&mut self) {
        self.follower.abort();
    }
}

impl Inner {
    fn current(&self) -> Option<SessionHelper1Proxy<'static>> {
        self.helper
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, helper: Option<SessionHelper1Proxy<'static>>) {
        *self.helper.lock().unwrap_or_else(PoisonError::into_inner) = helper;
    }

    fn helper(&self) -> Result<SessionHelper1Proxy<'static>, PlatformError> {
        self.current().ok_or_else(|| {
            PlatformError::Unavailable("Wye's GNOME Shell extension is not running".to_owned())
        })
    }

    /// Tell the extension whether to send changes; logged, never fatal.
    async fn tell_watching(&self, helper: &SessionHelper1Proxy<'static>) {
        let wanted = self.watching.load(Ordering::Relaxed);
        if let Err(error) = helper.watch_clipboard(wanted).await {
            tracing::warn!(%error, wanted, "cannot switch the Shell's clipboard watch");
        }
    }
}

/// Track the extension's bus name for as long as the helper lives.
async fn follow(inner: Arc<Inner>) {
    if let Err(error) = follow_owner(&inner).await {
        tracing::warn!(%error, "cannot follow the GNOME Shell extension");
    }
    inner.set(None);
}

async fn follow_owner(inner: &Arc<Inner>) -> zbus::Result<()> {
    let bus = DBusProxy::new(&inner.connection).await?;
    let mut owners = bus
        .receive_name_owner_changed_with_args(&[(0, GNOME_BUS_NAME)])
        .await?;
    let mut owner = bus
        .get_name_owner(BusName::try_from(GNOME_BUS_NAME)?)
        .await
        .ok()
        .map(|owner| owner.to_string());
    loop {
        let listener = match owner.take() {
            Some(owner) => attach(inner, &owner).await,
            None => None,
        };
        let Some(change) = owners.next().await else {
            return Ok(());
        };
        if let Some(listener) = listener {
            listener.abort();
        }
        inner.set(None);
        owner = change.args()?.new_owner().as_ref().map(ToString::to_string);
    }
}

/// Use the helper at `owner` when it serves the interface, and pass on its
/// clipboard changes until it goes.
async fn attach(inner: &Arc<Inner>, owner: &str) -> Option<JoinHandle<()>> {
    match connect(&inner.connection, owner).await {
        Ok(Some((helper, mut changes))) => {
            // Set first: a watch switched while telling is then told too.
            inner.set(Some(helper.clone()));
            inner.tell_watching(&helper).await;
            tracing::info!(owner, "the GNOME Shell extension is the session helper");
            let inner = Arc::clone(inner);
            Some(tokio::spawn(async move {
                while let Some(signal) = changes.next().await {
                    let Ok(args) = signal.args() else {
                        continue;
                    };
                    // Sent only while asked for; a change after the service
                    // stopped watching is dropped all the same.
                    if !inner.watching.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let Some(text) = single_line_text(args.text()) {
                        // No receiver yet is fine: nobody is rewriting.
                        let _ = inner.changes.send(text);
                    }
                }
            }))
        }
        Ok(None) => {
            tracing::info!(owner, "the GNOME Shell extension serves no session helper");
            None
        }
        Err(error) => {
            tracing::warn!(%error, owner, "cannot reach the GNOME Shell extension");
            None
        }
    }
}

type Changes = wye_api::proxy::ClipboardChangedStream;

async fn connect(
    connection: &zbus::Connection,
    owner: &str,
) -> zbus::Result<Option<(SessionHelper1Proxy<'static>, Changes)>> {
    let introspection = IntrospectableProxy::builder(connection)
        .destination(owner.to_owned())?
        .path(GNOME_OBJECT_PATH)?
        .build()
        .await?
        .introspect()
        .await?;
    if !introspection.contains(&format!("\"{SESSION_HELPER_INTERFACE}\"")) {
        return Ok(None);
    }
    let helper = SessionHelper1Proxy::builder(connection)
        .destination(owner.to_owned())?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    // Subscribed before the watch is switched on, so no change is missed.
    let changes = helper.receive_clipboard_changed().await?;
    Ok(Some((helper, changes)))
}

/// `work`, or [`PlatformError::Timeout`] after `deadline`.
async fn within<T>(
    deadline: Duration,
    work: impl Future<Output = Result<T, wye_api::Error>>,
) -> Result<T, PlatformError> {
    match tokio::time::timeout(deadline, work).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(PlatformError::Failed(error.to_string())),
        Err(_) => Err(PlatformError::Timeout(deadline)),
    }
}

/// `work` on the helper, logging and forgetting a failure: the queries
/// below answer "unknown" rather than fail.
async fn query<T, F>(
    inner: &Inner,
    what: &str,
    work: impl FnOnce(SessionHelper1Proxy<'static>) -> F,
) -> Option<T>
where
    F: Future<Output = Result<T, wye_api::Error>>,
{
    let helper = inner.current()?;
    within(QUERY_TIMEOUT, work(helper))
        .await
        .inspect_err(|error| tracing::debug!(%error, "the Shell did not tell {what}"))
        .ok()
}

/// The modifiers in `names` that Wye knows; unknown names are left out.
#[must_use]
pub fn modifiers_from(names: &[String]) -> Vec<Modifier> {
    names.iter().filter_map(|name| name.parse().ok()).collect()
}

/// The focused app from the desktop ID the Shell reports; `None` when it
/// reports none.
#[must_use]
pub fn focused_from(desktop_id: &str) -> Option<FocusedApp> {
    let id = desktop_id.trim();
    (!id.is_empty()).then(|| FocusedApp {
        desktop_id: Some(id.to_owned()),
        ..FocusedApp::default()
    })
}

#[async_trait]
impl ClipboardProvider for ShellHelper {
    async fn read(&self) -> Result<Option<String>, PlatformError> {
        let helper = self.inner.helper()?;
        let text = within(CLIPBOARD_TIMEOUT, helper.read_clipboard()).await?;
        Ok(single_line_text(&text))
    }

    async fn write(&self, text: &str) -> Result<(), PlatformError> {
        let helper = self.inner.helper()?;
        within(CLIPBOARD_TIMEOUT, helper.write_clipboard(text)).await
    }

    fn watch(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.inner.changes.subscribe())
    }

    fn set_watching(&self, wanted: bool) {
        if self.inner.watching.swap(wanted, Ordering::Relaxed) == wanted {
            return;
        }
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            if let Some(helper) = inner.current() {
                inner.tell_watching(&helper).await;
            }
        });
    }

    fn capabilities(&self) -> ClipboardCapabilities {
        let mechanism = self.mechanism();
        ClipboardCapabilities {
            read: mechanism,
            watch: mechanism,
            write: mechanism,
        }
    }
}

#[async_trait]
impl ModifierSource for ShellHelper {
    async fn held(&self) -> Option<Vec<Modifier>> {
        query(&self.inner, "the held modifiers", |helper| async move {
            helper.query_modifiers().await
        })
        .await
        .map(|names| modifiers_from(&names))
    }

    fn mechanism(&self) -> Option<&'static str> {
        Self::mechanism(self)
    }
}

#[async_trait]
impl PointerSource for ShellHelper {
    async fn pointer(&self) -> Option<Placement> {
        let (x, y, output) = query(&self.inner, "the pointer", |helper| async move {
            helper.query_pointer().await
        })
        .await?;
        (!output.is_empty()).then_some(Placement { output, x, y })
    }

    fn mechanism(&self) -> Option<&'static str> {
        Self::mechanism(self)
    }
}

#[async_trait]
impl FocusSource for ShellHelper {
    async fn focused(&self) -> Option<FocusedApp> {
        let id = query(&self.inner, "the focused app", |helper| async move {
            helper.focused_app().await
        })
        .await?;
        focused_from(&id)
    }

    fn mechanism(&self) -> Option<&'static str> {
        Self::mechanism(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wyes_modifier_names_count() {
        let names = ["Shift", "Hyper", "Super"].map(str::to_owned);
        assert_eq!(
            modifiers_from(&names),
            vec![Modifier::Shift, Modifier::Super]
        );
    }

    #[test]
    fn an_empty_desktop_id_is_no_app() {
        assert_eq!(focused_from("  "), None);
        assert_eq!(
            focused_from("org.gnome.Ptyxis.desktop"),
            Some(FocusedApp {
                desktop_id: Some("org.gnome.Ptyxis.desktop".to_owned()),
                ..FocusedApp::default()
            })
        );
    }
}
