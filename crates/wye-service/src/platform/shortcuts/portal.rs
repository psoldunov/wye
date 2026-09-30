//! Global shortcuts through the `GlobalShortcuts` portal (KEY-40), over
//! ashpd.
//!
//! One long-lived session on a connection of its own: the host-app registry
//! (`org.freedesktop.host.portal.Registry.Register`) must name the app
//! before that connection makes any other portal call, so the service's
//! main connection cannot be used. On Plasma the shortcuts then appear, and
//! can be changed, in System Settings → Shortcuts under Wye.
//!
//! `BindShortcuts` may be answered only once per session, and the portal
//! may show a dialog before it answers, so binding runs in its own task and
//! binding again (a changed preference, `SetShortcut`) closes the session
//! and opens a new one. The desktop keeps what the user chose per app, so a
//! new session does not lose it; a preferred trigger only applies where the
//! user has not chosen one.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, ListShortcutsOptions,
    NewShortcut, Shortcut,
};
use ashpd::desktop::{CreateSessionOptions, Session};
use ashpd::{AppID, register_host_app_with_connection};
use async_trait::async_trait;
use futures_lite::StreamExt as _;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::{ACTIONS, ShortcutAction, action, trigger};
use crate::platform::{BoundShortcut, PlatformError, ShortcutProvider};

/// The app ID the portal files the shortcuts under; Wye's desktop entry.
pub const APP_ID: &str = "dev.soldunov.wye";

/// What `mechanism` reports.
const MECHANISM: &str = "portal";

/// How many presses a slow listener may fall behind.
const ACTIVATION_BUFFER: usize = 16;

/// Longest the portal may take to open a session or list shortcuts.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Longest `bind` waits for the portal's answer; a dialog the user leaves
/// open keeps binding in the background after that.
const BIND_WAIT: Duration = Duration::from_secs(20);

/// Stored bindings (KEY-03) by action ID; an action without one is asked
/// for without a preference.
pub type Preferred = BTreeMap<&'static str, String>;

/// The `GlobalShortcuts` portal.
pub struct PortalShortcuts {
    inner: Arc<Inner>,
    listener: JoinHandle<()>,
}

struct Inner {
    portal: GlobalShortcuts,
    /// The session and whether `BindShortcuts` was answered on it. Held
    /// across a whole bind, so binds run one at a time.
    live: tokio::sync::Mutex<Live>,
    preferred: Mutex<Preferred>,
    /// The bindings as the portal last reported them.
    bound: Mutex<Vec<BoundShortcut>>,
    activations: broadcast::Sender<String>,
}

struct Live {
    session: Session<GlobalShortcuts>,
    bound: bool,
}

impl PortalShortcuts {
    /// Register Wye with the portal on `connection` (which must be used for
    /// nothing else), open a session, start listening for presses and bind
    /// every action in the background with its `preferred` trigger.
    ///
    /// # Errors
    ///
    /// [`PlatformError::Unavailable`] when there is no `GlobalShortcuts`
    /// portal or it cannot open a session.
    pub async fn start(
        connection: zbus::Connection,
        preferred: Preferred,
    ) -> Result<Self, PlatformError> {
        register(&connection).await;
        let portal = within(GlobalShortcuts::with_connection(connection)).await?;
        let session = within(portal.create_session(CreateSessionOptions::default())).await?;
        let presses = within(portal.receive_activated()).await?;
        let inner = Arc::new(Inner {
            portal,
            live: tokio::sync::Mutex::new(Live {
                session,
                bound: false,
            }),
            preferred: Mutex::new(preferred),
            bound: Mutex::new(Vec::new()),
            activations: broadcast::Sender::new(ACTIVATION_BUFFER),
        });
        let listener = tokio::spawn(forward_presses(presses, inner.activations.clone()));
        tokio::spawn(Inner::bind_logged(inner.clone()));
        Ok(Self { inner, listener })
    }

    /// Bind again with the current preferences and wait a while for the
    /// portal's answer.
    async fn rebind(&self) -> Result<(), PlatformError> {
        let task = tokio::spawn(Inner::bind(self.inner.clone()));
        match tokio::time::timeout(BIND_WAIT, task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => Err(PlatformError::Failed(format!(
                "binding the shortcuts stopped: {error}"
            ))),
            // The portal is still waiting for the user; the task goes on.
            Err(_) => Ok(()),
        }
    }
}

impl Drop for PortalShortcuts {
    fn drop(&mut self) {
        self.listener.abort();
    }
}

impl std::fmt::Debug for PortalShortcuts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PortalShortcuts")
            .field("bound", &*lock(&self.inner.bound))
            .finish_non_exhaustive()
    }
}

impl Inner {
    async fn bind_logged(self: Arc<Self>) {
        if let Err(error) = self.bind().await {
            tracing::warn!(%error, "cannot bind the global shortcuts");
        }
    }

    /// `BindShortcuts` for every action, on a new session when the current
    /// one has already been bound.
    async fn bind(self: Arc<Self>) -> Result<(), PlatformError> {
        let mut live = self.live.lock().await;
        if live.bound {
            if let Err(error) = live.session.close().await {
                tracing::debug!(%error, "cannot close the old shortcuts session");
            }
            live.session =
                within(self.portal.create_session(CreateSessionOptions::default())).await?;
            live.bound = false;
        }
        let shortcuts = new_shortcuts(&lock(&self.preferred));
        let request = self
            .portal
            .bind_shortcuts(
                &live.session,
                &shortcuts,
                None,
                BindShortcutsOptions::default(),
            )
            .await
            .map_err(|error| failed(&error))?;
        live.bound = true;
        let answer = request.response().map_err(|error| failed(&error))?;
        self.remember(answer.shortcuts());
        Ok(())
    }

    fn remember(&self, shortcuts: &[Shortcut]) {
        *lock(&self.bound) = reported(shortcuts);
    }
}

#[async_trait]
impl ShortcutProvider for PortalShortcuts {
    /// Asks the portal (`ListShortcuts`); while a bind waits for the user,
    /// or when the portal does not answer, the last report.
    async fn bindings(&self) -> Result<Vec<BoundShortcut>, PlatformError> {
        let Ok(live) = self.inner.live.try_lock() else {
            return Ok(lock(&self.inner.bound).clone());
        };
        if live.bound {
            let listed = within(async {
                self.inner
                    .portal
                    .list_shortcuts(&live.session, ListShortcutsOptions::default())
                    .await?
                    .response()
            })
            .await;
            match listed {
                Ok(listed) => self.inner.remember(listed.shortcuts()),
                Err(error) => tracing::debug!(%error, "cannot list the shortcuts"),
            }
        }
        Ok(lock(&self.inner.bound).clone())
    }

    async fn bind(&self, action_id: &str, trigger: &str) -> Result<(), PlatformError> {
        let action = action(action_id).ok_or_else(|| {
            PlatformError::Failed(format!("there is no shortcut action {action_id:?}"))
        })?;
        {
            let mut preferred = lock(&self.inner.preferred);
            if trigger.is_empty() {
                preferred.remove(action.id);
            } else {
                preferred.insert(action.id, trigger.to_owned());
            }
        }
        self.rebind().await
    }

    /// The portal's own dialog (KEY-40 "Change…"); needs version 2.
    async fn configure(&self) -> Result<(), PlatformError> {
        let live = self.inner.live.lock().await;
        self.inner
            .portal
            .configure_shortcuts(&live.session, None, ConfigureShortcutsOptions::default())
            .await
            .map_err(|error| {
                PlatformError::Unavailable(format!(
                    "the shortcuts portal cannot open its settings: {error}"
                ))
            })
    }

    fn activations(&self) -> broadcast::Receiver<String> {
        self.inner.activations.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// Name Wye to the portal. Portals before the host registry (1.19) name
/// host apps from their systemd unit instead, so a failure only costs the
/// name shown in the desktop's settings.
async fn register(connection: &zbus::Connection) {
    let registered = match AppID::try_from(APP_ID) {
        Ok(app_id) => {
            tokio::time::timeout(
                CALL_TIMEOUT,
                register_host_app_with_connection(connection.clone(), app_id),
            )
            .await
        }
        Err(error) => {
            tracing::warn!(%error, "Wye's app ID is not valid for the portal");
            return;
        }
    };
    match registered {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::info!(%error, "cannot register Wye with the portal"),
        Err(_) => tracing::info!("the portal registry did not answer"),
    }
}

/// Forward presses of Wye's own actions. The connection is Wye's alone and
/// the portal sends a session's signals only to its owner, so every press
/// on it is one of Wye's.
async fn forward_presses(
    mut presses: impl futures_lite::Stream<Item = ashpd::desktop::global_shortcuts::Activated> + Unpin,
    activations: broadcast::Sender<String>,
) {
    while let Some(press) = presses.next().await {
        let id = press.shortcut_id();
        if action(id).is_none() {
            tracing::debug!(id, "press of an unknown shortcut");
            continue;
        }
        // Nobody listening is a service still starting; the press is lost.
        let _ = activations.send(id.to_owned());
    }
}

/// What `BindShortcuts` is asked for: every action, with its preference.
fn new_shortcuts(preferred: &Preferred) -> Vec<NewShortcut> {
    ACTIONS
        .iter()
        .map(|action| new_shortcut(action, preferred.get(action.id).map(String::as_str)))
        .collect()
}

fn new_shortcut(action: &ShortcutAction, stored: Option<&str>) -> NewShortcut {
    let preferred = stored.and_then(trigger::portal_trigger);
    NewShortcut::new(action.id, action.description).preferred_trigger(preferred.as_deref())
}

/// The portal's report as [`BoundShortcut`]s; an empty trigger description
/// is an unbound shortcut.
fn reported(shortcuts: &[Shortcut]) -> Vec<BoundShortcut> {
    shortcuts
        .iter()
        .map(|shortcut| BoundShortcut {
            action: shortcut.id().to_owned(),
            trigger: Some(shortcut.trigger_description().trim())
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
        })
        .collect()
}

async fn within<T>(
    call: impl Future<Output = Result<T, ashpd::Error>>,
) -> Result<T, PlatformError> {
    tokio::time::timeout(CALL_TIMEOUT, call)
        .await
        .map_err(|_| PlatformError::Timeout(CALL_TIMEOUT))?
        .map_err(|error| PlatformError::Unavailable(format!("shortcuts portal: {error}")))
}

fn failed(error: &ashpd::Error) -> PlatformError {
    PlatformError::Failed(format!("shortcuts portal: {error}"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
