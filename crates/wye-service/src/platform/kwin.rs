//! `KWin` helper: pointer position and the active window on Plasma (PICK-02,
//! source-app step 4 in `docs/spec/13-linux-platform.md`).
//!
//! A client on Wayland can read neither the pointer nor the focused window,
//! but a `KWin` script can. Each query renders `data/kwin/wye-query.js` with
//! a fresh nonce, loads it through `org.kde.kwin.Scripting.loadScript(path,
//! "wye-query-<nonce>")`, starts it, and waits up to [`TIMEOUT`] for the
//! script to call `dev.soldunov.wye.KWin1.Report` back with the same nonce;
//! then unloads it. Only the compositor's own connection may answer. Without
//! an answer the picker is centred and the source app is unknown.

pub mod script;
pub mod scripting;

use std::collections::HashMap;
use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::oneshot;
use wye_api::Error;
use wye_api::names::{KWIN_INTERFACE, OBJECT_PATH};
use wye_api::picker::Placement;

use self::script::ReplyTo;
use self::scripting::{KWinScripting, ScriptHost};
use super::{FocusSource, FocusedApp, PlatformError, PointerSource};
use crate::api::Caller;
use crate::context::ServiceContext;

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "kwin-script";

/// How long a query may take, from loading the script to its answer.
pub const TIMEOUT: Duration = Duration::from_millis(150);

/// How long unloading a finished script may take.
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);

/// A link's source lookup and its picker placement come moments apart; one
/// query answers both.
const REUSE_FOR: Duration = Duration::from_millis(250);

/// One answer from the `KWin` script (`KWin1.Report`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KWinReport {
    /// The query this answers.
    pub nonce: String,
    /// Pointer position, relative to its output, and the output.
    pub pointer: Placement,
    /// Active window's process ID; 0 when none.
    pub pid: u32,
    /// Active window's desktop file name; empty when none.
    pub desktop_file: String,
    /// Active window's resource class; empty when none.
    pub resource_class: String,
}

impl KWinReport {
    /// Where the pointer is; `None` when the script found no output under
    /// it.
    #[must_use]
    pub fn placement(&self) -> Option<Placement> {
        (!self.pointer.output.is_empty()).then(|| self.pointer.clone())
    }

    /// The active window's app; `None` when there is no active window.
    #[must_use]
    pub fn focused(&self) -> Option<FocusedApp> {
        let app = FocusedApp {
            pid: (self.pid != 0).then_some(self.pid),
            desktop_id: non_empty(&self.desktop_file),
            resource_class: non_empty(&self.resource_class),
        };
        (app != FocusedApp::default()).then_some(app)
    }
}

/// Handle `KWin1.Report`: hand the answer to the query waiting for it.
///
/// A report nobody waits for (it came after the timeout) is dropped.
///
/// # Errors
///
/// `InvalidArgs` when a query waits for `report.nonce` but the caller is
/// not the compositor that was asked.
pub(crate) fn report(
    ctx: &ServiceContext,
    caller: &Caller,
    report: KWinReport,
) -> Result<(), Error> {
    ctx.platform()
        .kwin_reports
        .deliver(caller.sender.as_deref(), report)
        .map(|delivery| {
            if delivery == Delivery::Unexpected {
                tracing::debug!("dropped a KWin report nobody waits for");
            }
        })
}

/// Whether [`Reports::deliver`] found a waiting query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The query got its answer.
    Delivered,
    /// No query waits for this nonce (late, or never asked).
    Unexpected,
}

/// Queries waiting for their `KWin1.Report`, by nonce. Shared by the
/// helper that asks and the bus object that receives.
#[derive(Debug, Clone, Default)]
pub struct Reports {
    pending: Arc<Mutex<HashMap<String, Pending>>>,
}

#[derive(Debug)]
struct Pending {
    /// Unique bus name the answer must come from.
    from: String,
    answer: oneshot::Sender<KWinReport>,
}

impl Reports {
    /// No queries waiting.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Wait for the answer to `nonce`, which only `from` may give.
    #[must_use]
    pub fn expect(&self, nonce: &str, from: &str) -> oneshot::Receiver<KWinReport> {
        let (answer, receiver) = oneshot::channel();
        let pending = Pending {
            from: from.to_owned(),
            answer,
        };
        self.lock().insert(nonce.to_owned(), pending);
        receiver
    }

    /// Stop waiting for `nonce`.
    pub fn forget(&self, nonce: &str) {
        self.lock().remove(nonce);
    }

    /// How many queries are waiting.
    #[must_use]
    pub fn waiting(&self) -> usize {
        self.lock().len()
    }

    /// Hand `report` from `sender` to the query waiting for its nonce.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when the query waits for another sender; it keeps
    /// waiting.
    pub fn deliver(&self, sender: Option<&str>, report: KWinReport) -> Result<Delivery, Error> {
        let mut pending = self.lock();
        let Some(waiting) = pending.get(&report.nonce) else {
            return Ok(Delivery::Unexpected);
        };
        if sender != Some(waiting.from.as_str()) {
            return Err(Error::invalid_args(
                "KWin1.Report is only accepted from the compositor",
            ));
        }
        let Some(waiting) = pending.remove(&report.nonce) else {
            return Ok(Delivery::Unexpected);
        };
        // The receiver is gone when the query timed out a moment ago.
        Ok(match waiting.answer.send(report) {
            Ok(()) => Delivery::Delivered,
            Err(_) => Delivery::Unexpected,
        })
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Pending>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Pointer and focus from a one-shot `KWin` script.
#[derive(Debug)]
pub struct KWinHelper {
    host: Arc<dyn ScriptHost>,
    reports: Reports,
    reply_to: ReplyTo,
    dir: PathBuf,
    timeout: Duration,
    last: tokio::sync::Mutex<Option<(Instant, Option<KWinReport>)>>,
}

impl KWinHelper {
    /// Load scripts into `host`, written to `dir`, answering to `reply_to`
    /// through `reports`.
    #[must_use]
    pub fn new(
        host: Arc<dyn ScriptHost>,
        reports: Reports,
        reply_to: ReplyTo,
        dir: PathBuf,
    ) -> Self {
        Self {
            host,
            reports,
            reply_to,
            dir,
            timeout: TIMEOUT,
            last: tokio::sync::Mutex::new(None),
        }
    }

    /// The same helper with another timeout (tests).
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// The helper for this session: `KWin` on `connection`, answering to
    /// `connection`'s unique name, scripts in `$XDG_RUNTIME_DIR/wye/kwin`.
    /// `None` when `KWin` is not on the bus or there is no runtime
    /// directory.
    pub async fn detect(connection: &zbus::Connection, reports: Reports) -> Option<Self> {
        let host = KWinScripting::new(connection.clone());
        host.owner().await?;
        let service = connection.unique_name()?.to_string();
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())?
            .join("wye")
            .join("kwin");
        let reply_to = ReplyTo {
            service,
            path: OBJECT_PATH.to_owned(),
            interface: KWIN_INTERFACE.to_owned(),
        };
        Some(Self::new(Arc::new(host), reports, reply_to, dir))
    }

    /// The latest answer, asking `KWin` unless one came moments ago; `None`
    /// when it did not answer.
    pub async fn query(&self) -> Option<KWinReport> {
        let mut last = self.last.lock().await;
        if let Some((at, report)) = last.as_ref()
            && at.elapsed() < REUSE_FOR
        {
            return report.clone();
        }
        let started = Instant::now();
        let report = self
            .ask()
            .await
            .inspect_err(|error| tracing::info!(%error, "no answer from KWin"))
            .ok();
        tracing::debug!(elapsed = ?started.elapsed(), answered = report.is_some(), "KWin query");
        *last = Some((Instant::now(), report.clone()));
        report
    }

    async fn ask(&self) -> Result<KWinReport, PlatformError> {
        let nonce = nonce();
        let name = script::name(&nonce);
        let text = script::render(&self.reply_to, &nonce)?;
        let path = script::write(&self.dir, &name, &text)?;
        let outcome = tokio::time::timeout(self.timeout, self.exchange(&nonce, &name, &path))
            .await
            .unwrap_or(Err(PlatformError::Timeout(self.timeout)));
        self.clean_up(&nonce, &name, &path).await;
        outcome
    }

    /// Load and start the script, then wait for its answer.
    async fn exchange(
        &self,
        nonce: &str,
        name: &str,
        path: &Path,
    ) -> Result<KWinReport, PlatformError> {
        let from = self
            .host
            .owner()
            .await
            .ok_or_else(|| PlatformError::Unavailable("KWin is not on the bus".to_owned()))?;
        let answer = self.reports.expect(nonce, &from);
        self.host.load(path, name).await?;
        self.host.start().await?;
        answer
            .await
            .map_err(|_| PlatformError::Failed("the query was dropped".to_owned()))
    }

    async fn clean_up(&self, nonce: &str, name: &str, path: &Path) {
        self.reports.forget(nonce);
        match tokio::time::timeout(CLEANUP_TIMEOUT, self.host.unload(name)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(%error, name, "cannot unload the KWin script"),
            Err(_) => tracing::warn!(name, "KWin did not unload the script in time"),
        }
        script::remove(path);
    }
}

#[async_trait]
impl PointerSource for KWinHelper {
    async fn pointer(&self) -> Option<Placement> {
        self.query().await?.placement()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

#[async_trait]
impl FocusSource for KWinHelper {
    async fn focused(&self) -> Option<FocusedApp> {
        self.query().await?.focused()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// The pointer is never known.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPointer;

#[async_trait]
impl PointerSource for NoPointer {
    async fn pointer(&self) -> Option<Placement> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

/// A fresh, hard-to-guess nonce: 32 hex digits from the standard library's
/// randomly keyed hasher and a counter. The sender check in
/// [`Reports::deliver`] is what keeps other clients out; the nonce only
/// pairs answers with queries.
fn nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let half = |salt: u64| {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(count);
        hasher.write_u64(salt);
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(0), half(1))
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests;
