//! The service's shared state, handed to every D-Bus handler.
//!
//! Each topic in [`crate::api`] owns a `State` type in its own file; the
//! context holds one of each, so a topic can grow its state without touching
//! this file.

use std::future::Future;
use std::sync::{Arc, OnceLock};

use tokio::sync::watch;

use crate::api;
use crate::platform::Platform;

/// A cheap, cloneable handle to the running service.
#[derive(Debug, Clone)]
pub struct ServiceContext {
    inner: Arc<Inner>,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the topic states are read through accessors no stub calls yet"
)]
struct Inner {
    platform: Platform,
    connection: OnceLock<zbus::Connection>,
    shutdown: watch::Sender<bool>,
    clipboard: api::clipboard::State,
    config: api::config::State,
    default_browser: api::default_browser::State,
    history: api::history::State,
    inventory: api::inventory::State,
    link: api::link::State,
    picker: api::picker::State,
    rules: api::rules::State,
    scripts: api::scripts::State,
    shortcuts: api::shortcuts::State,
    state: api::state::State,
    tray: api::tray::State,
    windows: api::windows::State,
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
            rules: api::rules::State::new(&platform),
            scripts: api::scripts::State::new(&platform),
            shortcuts: api::shortcuts::State::new(&platform),
            state: api::state::State::new(&platform),
            tray: api::tray::State::new(&platform),
            windows: api::windows::State::new(&platform),
            platform,
            connection: OnceLock::new(),
            shutdown: watch::Sender::new(false),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// The session integrations.
    #[must_use]
    pub fn platform(&self) -> &Platform {
        &self.inner.platform
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

/// Each topic's state, for that topic's functions.
#[allow(
    dead_code,
    reason = "a topic reads its state once it has any; the stubs have none yet"
)]
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

    /// Rules topic state.
    #[must_use]
    pub(crate) fn rules(&self) -> &api::rules::State {
        &self.inner.rules
    }

    /// Scripts topic state.
    #[must_use]
    pub(crate) fn scripts(&self) -> &api::scripts::State {
        &self.inner.scripts
    }

    /// Shortcuts topic state.
    #[must_use]
    pub(crate) fn shortcuts(&self) -> &api::shortcuts::State {
        &self.inner.shortcuts
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

    /// Windows topic state.
    #[must_use]
    pub(crate) fn windows(&self) -> &api::windows::State {
        &self.inner.windows
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
