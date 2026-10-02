//! Detecting the session's integrations for `wye service`, without ever
//! holding up the bus name.
//!
//! [`base`] builds what needs no answer from anyone (notifications,
//! launching, scopes, the tray item, HTTP), so the service can serve and
//! claim `dev.soldunov.wye` at once (DEF-04). The rest is asked for after
//! that, each with a deadline: [`probes`] (screen lock, clipboard, held
//! modifiers, pointer and focus) and [`shortcuts`] (the portal, which may
//! still be starting at login). Whatever does not answer in time is
//! unavailable until the next start.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::clipboard::{self, NoClipboard};
use super::gnome_shell::{self, ShellHelper};
use super::kwin::Reports;
use super::lock::{NoLockMonitor, SessionLockMonitor};
use super::modifiers::{self, NoModifiers};
use super::notify::DesktopNotifier;
use super::scope::{SpawnLauncher, SystemdScopes};
use super::shortcuts::{self, NoShortcuts};
use super::sni::KsniNotifier;
use super::{
    ClipboardProvider, FocusSource, LockMonitor, ModifierSource, Platform, PointerSource,
    ShortcutProvider, focus,
};

/// Longest each session probe may take to set up.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(3);

/// Longest the global-shortcuts portal may take to open a session; it is
/// often still being activated right after login.
pub const SHORTCUTS_DEADLINE: Duration = Duration::from_secs(20);

/// The integrations that need no answer from another process, over
/// `session`; the rest unavailable until detected.
#[must_use]
pub fn base(session: &zbus::Connection) -> Platform {
    Platform {
        notifier: Arc::new(DesktopNotifier::new(session.clone())),
        launcher: Arc::new(SpawnLauncher),
        scope: Arc::new(SystemdScopes::new(session.clone())),
        sni: Arc::new(KsniNotifier::new()),
        http: Arc::new(super::http::UreqClient::new()),
        ..Platform::unavailable()
    }
}

/// What [`probes`] found.
pub struct Probes {
    lock: Arc<dyn LockMonitor>,
    clipboard: Arc<dyn ClipboardProvider>,
    modifiers: Arc<dyn ModifierSource>,
    pointer: Arc<dyn PointerSource>,
    focus: Arc<dyn FocusSource>,
}

impl Probes {
    /// `platform` with these probes.
    #[must_use]
    pub fn apply(self, platform: &Platform) -> Platform {
        Platform {
            lock: self.lock,
            clipboard: self.clipboard,
            modifiers: self.modifiers,
            pointer: self.pointer,
            focus: self.focus,
            ..platform.clone()
        }
    }

    /// GNOME: the Shell extension's helper for whatever the session
    /// offered no other way to do (Mutter has no data control, layer shell
    /// or pointer query). What another mechanism found is kept.
    #[must_use]
    pub fn or_shell_helper(self, helper: Arc<ShellHelper>) -> Self {
        Self {
            clipboard: if self.clipboard.capabilities().read.is_none() {
                Arc::clone(&helper) as _
            } else {
                self.clipboard
            },
            modifiers: self
                .modifiers
                .mechanism()
                .map_or_else(|| Arc::clone(&helper) as _, |_| self.modifiers),
            pointer: self
                .pointer
                .mechanism()
                .map_or_else(|| Arc::clone(&helper) as _, |_| self.pointer),
            focus: self
                .focus
                .mechanism()
                .map_or_else(|| helper as _, |_| self.focus),
            lock: self.lock,
        }
    }
}

/// Screen lock (logind and the screen saver), clipboard, held modifiers,
/// pointer and focus, all at once, each within [`PROBE_DEADLINE`]. `KWin`
/// scripts answer through `reports`.
pub async fn probes(session: &zbus::Connection, reports: &Reports) -> Probes {
    let lock = within(
        "the screen-lock state",
        PROBE_DEADLINE,
        lock(session),
        || Arc::new(NoLockMonitor::new()) as Arc<dyn LockMonitor>,
    );
    let clipboard = within(
        "the clipboard",
        PROBE_DEADLINE,
        clipboard::detect(session),
        || Arc::new(NoClipboard) as Arc<dyn ClipboardProvider>,
    );
    // `advanced.held-keys` is read from the service's configuration cache
    // for each link (ADV-11), not from the file here.
    let modifiers = within(
        "held modifiers",
        PROBE_DEADLINE,
        modifiers::detect(),
        || Arc::new(NoModifiers) as Arc<dyn ModifierSource>,
    );
    let pointer_and_focus = within(
        "the pointer and focus",
        PROBE_DEADLINE,
        focus::detect(session, reports),
        || {
            (
                Arc::new(super::kwin::NoPointer) as Arc<dyn PointerSource>,
                Arc::new(focus::NoFocus) as Arc<dyn FocusSource>,
            )
        },
    );
    let (lock, clipboard, modifiers, (pointer, focus)) =
        tokio::join!(lock, clipboard, modifiers, pointer_and_focus);
    let found = Probes {
        lock,
        clipboard,
        modifiers,
        pointer,
        focus,
    };
    // GNOME Shell (not every desktop that names GNOME in
    // XDG_CURRENT_DESKTOP): the extension's helper may start after the
    // service, so it is used whether it runs yet or not.
    let found = if gnome_shell::is_gnome_session(session).await {
        found.or_shell_helper(Arc::new(ShellHelper::start(session)))
    } else {
        found
    };
    tracing::info!(
        lock = found.lock.mechanism(),
        clipboard = ?found.clipboard.capabilities().read,
        held_keys = found.modifiers.mechanism(),
        pointer = found.pointer.mechanism(),
        focus = found.focus.mechanism(),
        "session probes"
    );
    found
}

/// Global shortcuts through the portal (KEY-40), preferring the triggers in
/// `config`, within [`SHORTCUTS_DEADLINE`].
pub async fn shortcuts(config: Option<&Path>) -> Arc<dyn ShortcutProvider> {
    let found = within(
        "the global shortcuts portal",
        SHORTCUTS_DEADLINE,
        // Boxed: the portal's future is large.
        Box::pin(shortcuts::detect(config)),
        || Arc::new(NoShortcuts::new()) as Arc<dyn ShortcutProvider>,
    )
    .await;
    tracing::info!(shortcuts = found.mechanism(), "global shortcuts");
    found
}

async fn lock(session: &zbus::Connection) -> Arc<dyn LockMonitor> {
    let system = zbus::Connection::system()
        .await
        .inspect_err(|error| tracing::info!(%error, "no system bus"))
        .ok();
    Arc::new(SessionLockMonitor::start(session, system.as_ref()).await)
}

/// `work`, or `fallback` when it takes longer than `deadline`.
async fn within<T>(
    what: &str,
    deadline: Duration,
    work: impl Future<Output = T>,
    fallback: impl FnOnce() -> T,
) -> T {
    if let Ok(found) = tokio::time::timeout(deadline, work).await {
        found
    } else {
        tracing::warn!(
            ?deadline,
            "{what} did not answer in time; unavailable until restart"
        );
        fallback()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_step_that_never_answers_falls_back_at_its_deadline() {
        let started = tokio::time::Instant::now();
        let found = within(
            "a hung service",
            PROBE_DEADLINE,
            std::future::pending::<u8>(),
            || 7,
        )
        .await;
        assert_eq!(found, 7);
        assert_eq!(started.elapsed(), PROBE_DEADLINE);
    }

    #[tokio::test]
    async fn a_step_that_answers_is_kept() {
        let found = within("a quick service", PROBE_DEADLINE, async { 1 }, || 7).await;
        assert_eq!(found, 1);
    }
}
