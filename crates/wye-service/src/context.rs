//! The service's shared state, handed to every D-Bus handler.
//!
//! Each topic in [`crate::api`] owns a `State` type in its own file; the
//! context holds one of each, so a topic can grow its state without touching
//! this file.

use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

use tokio::sync::watch;

use crate::api;
use crate::api::link::Environment;
use crate::platform::Platform;

/// Set to `1` when something else starts Wye at login (the Nix modules'
/// systemd unit): the service then leaves the autostart entry alone
/// (GEN-01).
pub const LOGIN_MANAGED_ENV: &str = "WYE_LOGIN_MANAGED";

/// The session's files, or `None` when there is no home directory.
pub(crate) type SharedEnvironment = Option<Arc<Environment>>;

/// A cheap, cloneable handle to the running service.
#[derive(Debug, Clone)]
pub struct ServiceContext {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// Replaced once, when the session's integrations are detected after
    /// the bus name is claimed (`run`).
    platform: RwLock<Arc<Platform>>,
    connection: OnceLock<zbus::Connection>,
    shutdown: watch::Sender<bool>,
    /// Where the configuration, state, history and apps are; topics that
    /// cache files reload when it changes.
    environment: watch::Sender<SharedEnvironment>,
    /// The `wye` executable the autostart entry runs (GEN-01), when set
    /// explicitly; otherwise it is looked up.
    wye_executable: RwLock<Option<PathBuf>>,
    /// Starting at login is managed outside Wye ([`LOGIN_MANAGED_ENV`]).
    login_managed: AtomicBool,
    clipboard: api::clipboard::State,
    config: api::config::State,
    default_browser: api::default_browser::State,
    history: api::history::State,
    inventory: api::inventory::State,
    link: api::link::State,
    picker: api::picker::State,
    scripts: api::scripts::State,
    state: api::state::State,
    tray: api::tray::State,
}

impl ServiceContext {
    /// A context over `platform`, not yet attached to a bus.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        let inner = Inner {
            clipboard: api::clipboard::State::new(&platform),
            config: api::config::State::new(&platform),
            default_browser: api::default_browser::State::new(&platform),
            history: api::history::State::new(&platform),
            inventory: api::inventory::State::new(&platform),
            link: api::link::State::new(&platform),
            picker: api::picker::State::new(&platform),
            scripts: api::scripts::State::new(&platform),
            state: api::state::State::new(&platform),
            tray: api::tray::State::new(&platform),
            platform: RwLock::new(Arc::new(platform)),
            connection: OnceLock::new(),
            shutdown: watch::Sender::new(false),
            environment: watch::Sender::new(session_environment()),
            wye_executable: RwLock::new(None),
            login_managed: AtomicBool::new(login_managed_from_env()),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// The session integrations.
    #[must_use]
    pub fn platform(&self) -> Arc<Platform> {
        self.inner
            .platform
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replace the integrations with what `update` makes of them, in one
    /// step: the session's, once detected. Tasks that subscribe to an
    /// integration start after it is in place.
    pub fn update_platform(&self, update: impl FnOnce(&Platform) -> Platform) {
        let mut platform = self
            .inner
            .platform
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        *platform = Arc::new(update(&platform));
    }

    /// The bus connection the service is served on, once it is.
    #[must_use]
    pub fn connection(&self) -> Option<&zbus::Connection> {
        self.inner.connection.get()
    }

    /// Remember the connection the service is served on. Only the first call
    /// counts; returns whether this one did.
    pub(crate) fn attach(&self, connection: &zbus::Connection) -> bool {
        self.inner.connection.set(connection.clone()).is_ok()
    }

    /// Work with the files `environment` names instead of the session's
    /// (tests point this at temporary directories). Every topic follows:
    /// caches reload and the watchers move.
    pub fn set_environment(&self, environment: Environment) {
        self.inner
            .environment
            .send_replace(Some(Arc::new(environment)));
    }

    /// The session's files.
    ///
    /// # Errors
    ///
    /// [`wye_api::Error::Failed`] when there is no home directory.
    pub(crate) fn environment(&self) -> api::Result<Arc<Environment>> {
        self.inner
            .environment
            .borrow()
            .clone()
            .ok_or_else(|| wye_api::Error::failed("the service cannot find your home directory"))
    }

    /// The environment now and every later change.
    pub(crate) fn environment_changes(&self) -> watch::Receiver<SharedEnvironment> {
        self.inner.environment.subscribe()
    }

    /// Whether starting at login is managed outside Wye (GEN-01).
    #[must_use]
    pub fn login_managed(&self) -> bool {
        self.inner.login_managed.load(Ordering::Relaxed)
    }

    /// Treat starting at login as managed outside Wye, or not (tests; the
    /// service reads [`LOGIN_MANAGED_ENV`]).
    pub fn set_login_managed(&self, managed: bool) {
        self.inner.login_managed.store(managed, Ordering::Relaxed);
    }

    /// Name the `wye` executable the autostart entry runs (GEN-01) instead
    /// of looking it up on `PATH` (tests point it at a fixture).
    pub fn set_wye_executable(&self, path: PathBuf) {
        *self
            .inner
            .wye_executable
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(path);
    }

    /// The executable set with [`ServiceContext::set_wye_executable`].
    pub(crate) fn wye_executable_override(&self) -> Option<PathBuf> {
        self.inner
            .wye_executable
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Ask the service to stop (`Quit`, TRAY-17).
    pub fn request_shutdown(&self) {
        self.inner.shutdown.send_replace(true);
    }

    /// Whether [`ServiceContext::request_shutdown`] was called.
    #[must_use]
    pub fn shutdown_requested(&self) -> bool {
        *self.inner.shutdown.borrow()
    }

    /// Resolves once [`ServiceContext::request_shutdown`] is called.
    pub fn until_shutdown(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut requested = self.inner.shutdown.subscribe();
        async move {
            // The sender lives as long as the context; an error means the
            // service is gone, which is a shutdown too.
            let _ = requested.wait_for(|requested| *requested).await;
        }
    }
}

/// Run blocking file work on a blocking thread.
///
/// # Errors
///
/// [`wye_api::Error::Failed`] when the work panicked.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> api::Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| wye_api::Error::failed(format!("file work stopped: {error}")))
}

/// Whether [`LOGIN_MANAGED_ENV`] says login start is managed elsewhere.
fn login_managed_from_env() -> bool {
    std::env::var_os(LOGIN_MANAGED_ENV).is_some_and(|value| value == "1")
}

/// The session's directories, from the process environment.
fn session_environment() -> SharedEnvironment {
    Environment::from_env()
        .inspect_err(|error| tracing::warn!(%error, "no session environment"))
        .ok()
        .map(Arc::new)
}

/// Each topic's state, for that topic's functions.
impl ServiceContext {
    /// Clipboard topic state.
    #[must_use]
    pub(crate) fn clipboard(&self) -> &api::clipboard::State {
        &self.inner.clipboard
    }

    /// Configuration topic state.
    #[must_use]
    pub(crate) fn config(&self) -> &api::config::State {
        &self.inner.config
    }

    /// Default-browser topic state.
    #[must_use]
    pub(crate) fn default_browser(&self) -> &api::default_browser::State {
        &self.inner.default_browser
    }

    /// History topic state.
    #[must_use]
    pub(crate) fn history(&self) -> &api::history::State {
        &self.inner.history
    }

    /// Inventory topic state.
    #[must_use]
    pub(crate) fn inventory(&self) -> &api::inventory::State {
        &self.inner.inventory
    }

    /// Link-routing topic state.
    #[must_use]
    pub(crate) fn link(&self) -> &api::link::State {
        &self.inner.link
    }

    /// Picker topic state.
    #[must_use]
    pub(crate) fn picker(&self) -> &api::picker::State {
        &self.inner.picker
    }

    /// Scripts topic state.
    #[must_use]
    pub(crate) fn scripts(&self) -> &api::scripts::State {
        &self.inner.scripts
    }

    /// Internal-state topic state (onboarding, UI state).
    #[must_use]
    pub(crate) fn state(&self) -> &api::state::State {
        &self.inner.state
    }

    /// Tray topic state.
    #[must_use]
    pub(crate) fn tray(&self) -> &api::tray::State {
        &self.inner.tray
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn a_shutdown_request_wakes_the_waiter() {
        let ctx = ServiceContext::new(Platform::unavailable());
        let waiting = tokio::spawn(ctx.until_shutdown());
        assert!(!ctx.shutdown_requested());
        ctx.request_shutdown();
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("woke up")
            .expect("did not panic");
        assert!(ctx.shutdown_requested());
    }

    #[tokio::test]
    async fn without_a_request_the_waiter_keeps_waiting() {
        let ctx = ServiceContext::new(Platform::unavailable());
        let raced = tokio::time::timeout(Duration::from_millis(50), ctx.until_shutdown()).await;
        assert!(raced.is_err(), "shut down without being asked to");
    }
}
