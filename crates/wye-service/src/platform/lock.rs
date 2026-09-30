//! Screen-lock state (PKS-05, PKS-07).
//!
//! [`SessionLockMonitor`] combines two sources and reports locked when
//! either says so: logind's `LockedHint` for the user's graphical session
//! (system bus; GNOME and Plasma both set it), and
//! `org.freedesktop.ScreenSaver.ActiveChanged` (session bus; Plasma and other
//! screen savers). A source that cannot be reached is skipped and logged.

use std::sync::{Arc, Mutex, PoisonError};

use futures_lite::StreamExt as _;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use zbus::zvariant::OwnedObjectPath;

use super::LockMonitor;

/// Mechanism names reported in `Status.capabilities.lockDetection`.
const LOGIND: &str = "logind";
const SCREENSAVER: &str = "screensaver";
const BOTH: &str = "logind+screensaver";

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/ScreenSaver"
)]
trait ScreenSaver {
    fn get_active(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn active_changed(&self, active: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LoginManager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.User",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1/user/self"
)]
trait LoginUser {
    #[zbus(property)]
    fn display(&self) -> zbus::Result<(String, OwnedObjectPath)>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait LoginSession {
    #[zbus(property)]
    fn locked_hint(&self) -> zbus::Result<bool>;
}

/// Which source a reading came from.
#[derive(Debug, Clone, Copy)]
enum Source {
    Logind,
    ScreenSaver,
}

/// The last reading of each source.
#[derive(Debug, Clone, Copy, Default)]
struct Sources {
    logind: bool,
    screensaver: bool,
}

impl Sources {
    const fn with(self, source: Source, locked: bool) -> Self {
        match source {
            Source::Logind => Self {
                logind: locked,
                ..self
            },
            Source::ScreenSaver => Self {
                screensaver: locked,
                ..self
            },
        }
    }

    /// Locked when any source says so.
    const fn any(self) -> bool {
        self.logind || self.screensaver
    }
}

/// The readings, and the combined state they publish.
#[derive(Debug)]
struct Readings {
    locked: Mutex<Sources>,
    state: watch::Sender<bool>,
}

impl Readings {
    fn set(&self, source: Source, locked: bool) {
        let mut readings = self.locked.lock().unwrap_or_else(PoisonError::into_inner);
        *readings = readings.with(source, locked);
        let any = readings.any();
        self.state.send_if_modified(|current| {
            let changed = *current != any;
            *current = any;
            changed
        });
    }
}

/// Lock state from logind and the screen saver.
#[derive(Debug)]
pub struct SessionLockMonitor {
    readings: Arc<Readings>,
    mechanism: Option<&'static str>,
    tasks: Vec<JoinHandle<()>>,
}

impl SessionLockMonitor {
    /// Read both sources and follow their changes. `system` is the system
    /// bus for logind; `None` skips it.
    pub async fn start(session: &zbus::Connection, system: Option<&zbus::Connection>) -> Self {
        let readings = Arc::new(Readings {
            locked: Mutex::new(Sources::default()),
            state: watch::Sender::new(false),
        });
        let logind = match system {
            Some(system) => follow_logind(system, Arc::clone(&readings))
                .await
                .inspect_err(|error| tracing::info!(%error, "no lock state from logind"))
                .ok(),
            None => None,
        };
        let screensaver = follow_screensaver(session, Arc::clone(&readings))
            .await
            .inspect_err(|error| tracing::info!(%error, "no lock state from the screen saver"))
            .ok();
        let mechanism = match (&logind, &screensaver) {
            (Some(_), Some(_)) => Some(BOTH),
            (Some(_), None) => Some(LOGIND),
            (None, Some(_)) => Some(SCREENSAVER),
            (None, None) => None,
        };
        Self {
            readings,
            mechanism,
            tasks: logind.into_iter().chain(screensaver).collect(),
        }
    }
}

impl Drop for SessionLockMonitor {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl LockMonitor for SessionLockMonitor {
    fn locked(&self) -> watch::Receiver<bool> {
        self.readings.state.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        self.mechanism
    }
}

async fn follow_screensaver(
    session: &zbus::Connection,
    readings: Arc<Readings>,
) -> zbus::Result<JoinHandle<()>> {
    let proxy = ScreenSaverProxy::new(session).await?;
    // Subscribed before the first read, so no change falls in between.
    let mut changes = proxy.receive_active_changed().await?;
    readings.set(Source::ScreenSaver, proxy.get_active().await?);
    Ok(tokio::spawn(async move {
        while let Some(signal) = changes.next().await {
            match signal.args() {
                Ok(args) => readings.set(Source::ScreenSaver, args.active),
                Err(error) => tracing::warn!(%error, "unreadable ActiveChanged"),
            }
        }
    }))
}

async fn follow_logind(
    system: &zbus::Connection,
    readings: Arc<Readings>,
) -> zbus::Result<JoinHandle<()>> {
    let path = session_path(system).await?;
    let session = LoginSessionProxy::builder(system)
        .path(path)?
        .build()
        .await?;
    let mut changes = session.receive_locked_hint_changed().await;
    readings.set(Source::Logind, session.locked_hint().await?);
    Ok(tokio::spawn(async move {
        while let Some(change) = changes.next().await {
            match change.get().await {
                Ok(locked) => readings.set(Source::Logind, locked),
                Err(error) => tracing::warn!(%error, "unreadable LockedHint"),
            }
        }
    }))
}

/// The service's own session, else the user's graphical session: a
/// systemd user service belongs to no session of its own.
async fn session_path(system: &zbus::Connection) -> zbus::Result<OwnedObjectPath> {
    let manager = LoginManagerProxy::new(system).await?;
    if let Ok(path) = manager.get_session_by_pid(std::process::id()).await {
        return Ok(path);
    }
    let user = LoginUserProxy::new(system).await?;
    let (_, path) = user.display().await?;
    Ok(path)
}

/// The screen is never reported locked.
#[derive(Debug)]
pub struct NoLockMonitor {
    state: watch::Sender<bool>,
}

impl NoLockMonitor {
    /// Always unlocked.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: watch::Sender::new(false),
        }
    }
}

impl Default for NoLockMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl LockMonitor for NoLockMonitor {
    fn locked(&self) -> watch::Receiver<bool> {
        self.state.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}
