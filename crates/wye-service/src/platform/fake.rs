//! Fakes of every platform trait, for tests on a private bus.
//!
//! [`FakePlatform`] keeps a handle to each fake so a test can set what the
//! "session" reports and read back what the service asked of it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use tokio::sync::{broadcast, watch};
use wye_api::context::Modifier;
use wye_api::picker::Placement;
use wye_api::tray::TrayMenu;

use super::kwin::scripting::ScriptHost;
use super::kwin::{KWinReport, Reports, script};
use super::{
    BoundShortcut, ClipboardCapabilities, ClipboardProvider, FocusSource, FocusedApp, HttpClient,
    HttpRequest, HttpResponse, LaunchCommand, Launcher, LockMonitor, ModifierSource, Notification,
    NotificationAction, Notifier, Platform, PlatformError, PointerSource, ScopeManager,
    ShortcutProvider, StatusNotifier, TrayEvent,
};

/// Mechanism name every fake reports.
pub const FAKE: &str = "fake";

/// Buffer of the fakes' broadcast channels.
const BUFFER: usize = 16;

/// Lock a fake's state; a panicking test must not hide the next one's data.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Append `item` to `log`; returns how many entries it holds now.
fn record<T: Clone>(log: &Mutex<Vec<T>>, item: &T) -> Result<u32, PlatformError> {
    let mut entries = lock(log);
    entries.push(item.clone());
    u32::try_from(entries.len()).map_err(|error| PlatformError::Failed(error.to_string()))
}

/// Every fake, and the [`Platform`] built from them.
#[derive(Debug, Clone, Default)]
pub struct FakePlatform {
    pub modifiers: Arc<FakeModifiers>,
    pub pointer: Arc<FakePointer>,
    pub focus: Arc<FakeFocus>,
    pub lock: Arc<FakeLock>,
    pub notifier: Arc<FakeNotifier>,
    pub launcher: Arc<FakeLauncher>,
    pub scope: Arc<FakeScopes>,
    pub clipboard: Arc<FakeClipboard>,
    pub shortcuts: Arc<FakeShortcuts>,
    pub http: Arc<FakeHttp>,
    pub sni: Arc<FakeStatusNotifier>,
}

impl FakePlatform {
    /// Fresh fakes: nothing held, unlocked, empty clipboard.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A [`Platform`] backed by these fakes.
    #[must_use]
    pub fn platform(&self) -> Platform {
        Platform {
            modifiers: Arc::clone(&self.modifiers) as _,
            pointer: Arc::clone(&self.pointer) as _,
            focus: Arc::clone(&self.focus) as _,
            lock: Arc::clone(&self.lock) as _,
            notifier: Arc::clone(&self.notifier) as _,
            launcher: Arc::clone(&self.launcher) as _,
            scope: Arc::clone(&self.scope) as _,
            clipboard: Arc::clone(&self.clipboard) as _,
            shortcuts: Arc::clone(&self.shortcuts) as _,
            http: Arc::clone(&self.http) as _,
            sni: Arc::clone(&self.sni) as _,
            kwin_reports: Reports::new(),
        }
    }
}

/// A tray item that records what it was asked to show; tests send its
/// events.
#[derive(Debug)]
pub struct FakeStatusNotifier {
    /// Each call in order: `Some(menu)` for `show`, `None` for `hide`.
    calls: Mutex<Vec<Option<TrayMenu>>>,
    events: broadcast::Sender<TrayEvent>,
}

impl Default for FakeStatusNotifier {
    fn default() -> Self {
        Self {
            calls: Mutex::default(),
            events: broadcast::channel(BUFFER).0,
        }
    }
}

impl FakeStatusNotifier {
    /// The menu shown now; `None` while hidden or never shown.
    pub fn shown(&self) -> Option<TrayMenu> {
        lock(&self.calls).last().cloned().flatten()
    }

    /// Every `show` (`Some`) and `hide` (`None`) so far.
    pub fn calls(&self) -> Vec<Option<TrayMenu>> {
        lock(&self.calls).clone()
    }

    /// What the user does with the item.
    pub fn send(&self, event: TrayEvent) {
        // No receiver yet is fine: the tray task has not started.
        let _ = self.events.send(event);
    }
}

#[async_trait]
impl StatusNotifier for FakeStatusNotifier {
    async fn show(&self, menu: &TrayMenu) -> Result<(), PlatformError> {
        lock(&self.calls).push(Some(menu.clone()));
        Ok(())
    }

    async fn hide(&self) {
        let mut calls = lock(&self.calls);
        if calls.last().is_some_and(Option::is_some) {
            calls.push(None);
        }
    }

    fn events(&self) -> broadcast::Receiver<TrayEvent> {
        self.events.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// What [`FakeScriptHost`] was asked to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptCalls {
    /// Each loaded script: its name and its text as it was on disk.
    pub loaded: Vec<(String, String)>,
    /// How often `start` was called.
    pub started: u32,
    /// Each unloaded script's name.
    pub unloaded: Vec<String>,
}

/// A `KWin` that answers each started script from `owner` with `answer`
/// (its nonce taken from the script's name), or stays silent.
#[derive(Debug, Default)]
pub struct FakeScriptHost {
    owner: Option<String>,
    reply: Option<(Reports, KWinReport)>,
    calls: Mutex<ScriptCalls>,
}

impl FakeScriptHost {
    /// A compositor on the bus as `owner` that loads scripts but never
    /// answers.
    #[must_use]
    pub fn silent(owner: &str) -> Self {
        Self {
            owner: Some(owner.to_owned()),
            ..Self::default()
        }
    }

    /// A compositor that answers every started script with `answer`
    /// through `reports`.
    #[must_use]
    pub fn answering(owner: &str, reports: Reports, answer: KWinReport) -> Self {
        Self {
            owner: Some(owner.to_owned()),
            reply: Some((reports, answer)),
            calls: Mutex::default(),
        }
    }

    /// What it was asked so far.
    pub fn calls(&self) -> ScriptCalls {
        lock(&self.calls).clone()
    }
}

#[async_trait]
impl ScriptHost for FakeScriptHost {
    async fn owner(&self) -> Option<String> {
        self.owner.clone()
    }

    async fn load(&self, path: &Path, name: &str) -> Result<(), PlatformError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| PlatformError::Failed(format!("{}: {error}", path.display())))?;
        lock(&self.calls).loaded.push((name.to_owned(), text));
        Ok(())
    }

    async fn start(&self) -> Result<(), PlatformError> {
        let last = {
            let mut calls = lock(&self.calls);
            calls.started += 1;
            calls.loaded.last().map(|(name, _)| name.clone())
        };
        let nonce = last
            .as_deref()
            .and_then(|name| name.strip_prefix(script::NAME_PREFIX));
        if let (Some(nonce), Some((reports, answer))) = (nonce, &self.reply) {
            let report = KWinReport {
                nonce: nonce.to_owned(),
                ..answer.clone()
            };
            reports
                .deliver(self.owner.as_deref(), report)
                .map_err(|error| PlatformError::Failed(error.to_string()))?;
        }
        Ok(())
    }

    async fn unload(&self, name: &str) -> Result<(), PlatformError> {
        lock(&self.calls).unloaded.push(name.to_owned());
        Ok(())
    }
}

/// Reports whatever modifiers the test set.
#[derive(Debug, Default)]
pub struct FakeModifiers {
    held: Mutex<Option<Vec<Modifier>>>,
}

impl FakeModifiers {
    /// What the next probe reports; `None` for unknown.
    pub fn set(&self, held: Option<Vec<Modifier>>) {
        *lock(&self.held) = held;
    }
}

#[async_trait]
impl ModifierSource for FakeModifiers {
    async fn held(&self) -> Option<Vec<Modifier>> {
        lock(&self.held).clone()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// Reports whatever pointer position the test set.
#[derive(Debug, Default)]
pub struct FakePointer {
    at: Mutex<Option<Placement>>,
}

impl FakePointer {
    /// Where the pointer is; `None` for unknown.
    pub fn set(&self, at: Option<Placement>) {
        *lock(&self.at) = at;
    }
}

#[async_trait]
impl PointerSource for FakePointer {
    async fn pointer(&self) -> Option<Placement> {
        lock(&self.at).clone()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// Reports whatever focused app the test set.
#[derive(Debug, Default)]
pub struct FakeFocus {
    app: Mutex<Option<FocusedApp>>,
}

impl FakeFocus {
    /// The focused app; `None` for unknown.
    pub fn set(&self, app: Option<FocusedApp>) {
        *lock(&self.app) = app;
    }
}

#[async_trait]
impl FocusSource for FakeFocus {
    async fn focused(&self) -> Option<FocusedApp> {
        lock(&self.app).clone()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// A lock state the test switches.
#[derive(Debug)]
pub struct FakeLock {
    state: watch::Sender<bool>,
}

impl FakeLock {
    /// Lock or unlock the fake screen.
    pub fn set(&self, locked: bool) {
        self.state.send_replace(locked);
    }
}

impl Default for FakeLock {
    fn default() -> Self {
        Self {
            state: watch::Sender::new(false),
        }
    }
}

impl LockMonitor for FakeLock {
    fn locked(&self) -> watch::Receiver<bool> {
        self.state.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// Records notifications; the test presses their buttons.
#[derive(Debug)]
pub struct FakeNotifier {
    shown: Mutex<Vec<Notification>>,
    closed: Mutex<Vec<u32>>,
    actions: broadcast::Sender<NotificationAction>,
}

impl FakeNotifier {
    /// Every notification shown so far, oldest first.
    #[must_use]
    pub fn shown(&self) -> Vec<Notification> {
        lock(&self.shown).clone()
    }

    /// The IDs of every notification withdrawn so far, oldest first.
    #[must_use]
    pub fn closed(&self) -> Vec<u32> {
        lock(&self.closed).clone()
    }

    /// Press `action` on notification `id` (IDs start at 1).
    pub fn press(&self, id: u32, action: &str) {
        // Nobody listening is a test that does not care about the answer.
        let _ = self.actions.send(NotificationAction {
            id,
            action: action.to_owned(),
        });
    }
}

impl Default for FakeNotifier {
    fn default() -> Self {
        Self {
            shown: Mutex::default(),
            closed: Mutex::default(),
            actions: broadcast::Sender::new(BUFFER),
        }
    }
}

#[async_trait]
impl Notifier for FakeNotifier {
    async fn notify(&self, notification: &Notification) -> Result<u32, PlatformError> {
        record(&self.shown, notification)
    }

    async fn close(&self, id: u32) -> Result<(), PlatformError> {
        lock(&self.closed).push(id);
        Ok(())
    }

    fn actions(&self) -> broadcast::Receiver<NotificationAction> {
        self.actions.subscribe()
    }
}

/// Records commands instead of starting them.
#[derive(Debug, Default)]
pub struct FakeLauncher {
    launched: Mutex<Vec<LaunchCommand>>,
}

/// First process ID the fake launcher hands out.
const FIRST_FAKE_PID: u32 = 10_000;

impl FakeLauncher {
    /// Every command launched so far, oldest first.
    #[must_use]
    pub fn launched(&self) -> Vec<LaunchCommand> {
        lock(&self.launched).clone()
    }
}

#[async_trait]
impl Launcher for FakeLauncher {
    async fn launch(&self, command: &LaunchCommand) -> Result<u32, PlatformError> {
        record(&self.launched, command).map(|count| FIRST_FAKE_PID + count)
    }
}

/// Records which processes were moved into scopes.
#[derive(Debug, Default)]
pub struct FakeScopes {
    adopted: Mutex<Vec<(u32, LaunchCommand)>>,
}

impl FakeScopes {
    /// Every process adopted so far, oldest first.
    #[must_use]
    pub fn adopted(&self) -> Vec<(u32, LaunchCommand)> {
        lock(&self.adopted).clone()
    }
}

#[async_trait]
impl ScopeManager for FakeScopes {
    async fn adopt(&self, pid: u32, command: &LaunchCommand) -> Result<(), PlatformError> {
        lock(&self.adopted).push((pid, command.clone()));
        Ok(())
    }
}

/// An in-memory clipboard; the test "copies" with [`FakeClipboard::copy`].
#[derive(Debug)]
pub struct FakeClipboard {
    text: Mutex<Option<String>>,
    written: Mutex<Vec<String>>,
    changes: broadcast::Sender<String>,
}

impl FakeClipboard {
    /// Put `text` on the clipboard as another app would.
    pub fn copy(&self, text: &str) {
        *lock(&self.text) = Some(text.to_owned());
        // Nobody watching is a test that does not care about changes.
        let _ = self.changes.send(text.to_owned());
    }

    /// Everything the service wrote, oldest first.
    #[must_use]
    pub fn written(&self) -> Vec<String> {
        lock(&self.written).clone()
    }
}

impl Default for FakeClipboard {
    fn default() -> Self {
        Self {
            text: Mutex::default(),
            written: Mutex::default(),
            changes: broadcast::Sender::new(BUFFER),
        }
    }
}

#[async_trait]
impl ClipboardProvider for FakeClipboard {
    async fn read(&self) -> Result<Option<String>, PlatformError> {
        Ok(lock(&self.text).clone())
    }

    async fn write(&self, text: &str) -> Result<(), PlatformError> {
        *lock(&self.text) = Some(text.to_owned());
        lock(&self.written).push(text.to_owned());
        Ok(())
    }

    fn watch(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.changes.subscribe())
    }

    fn capabilities(&self) -> ClipboardCapabilities {
        ClipboardCapabilities {
            read: Some(FAKE),
            watch: Some(FAKE),
            write: Some(FAKE),
        }
    }
}

/// Keeps bindings in memory; the test presses shortcuts.
#[derive(Debug)]
pub struct FakeShortcuts {
    bound: Mutex<Vec<BoundShortcut>>,
    configured: Mutex<usize>,
    activations: broadcast::Sender<String>,
}

impl FakeShortcuts {
    /// Press the shortcut of `action`.
    pub fn press(&self, action: &str) {
        // Nobody listening is a test that does not care about presses.
        let _ = self.activations.send(action.to_owned());
    }

    /// How often the configuration dialog was asked for.
    #[must_use]
    pub fn configured(&self) -> usize {
        *lock(&self.configured)
    }
}

impl Default for FakeShortcuts {
    fn default() -> Self {
        Self {
            bound: Mutex::default(),
            configured: Mutex::default(),
            activations: broadcast::Sender::new(BUFFER),
        }
    }
}

#[async_trait]
impl ShortcutProvider for FakeShortcuts {
    async fn bindings(&self) -> Result<Vec<BoundShortcut>, PlatformError> {
        Ok(lock(&self.bound).clone())
    }

    async fn bind(&self, action: &str, trigger: &str) -> Result<(), PlatformError> {
        let entry = BoundShortcut {
            action: action.to_owned(),
            trigger: (!trigger.is_empty()).then(|| trigger.to_owned()),
        };
        let mut bound = lock(&self.bound);
        let kept = bound
            .iter()
            .filter(|shortcut| shortcut.action != action)
            .cloned();
        *bound = kept.chain(std::iter::once(entry)).collect();
        Ok(())
    }

    async fn configure(&self) -> Result<(), PlatformError> {
        *lock(&self.configured) += 1;
        Ok(())
    }

    fn activations(&self) -> broadcast::Receiver<String> {
        self.activations.subscribe()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(FAKE)
    }
}

/// Answers from a table of URLs; unknown URLs fail.
#[derive(Debug, Default)]
pub struct FakeHttp {
    responses: Mutex<HashMap<String, HttpResponse>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl FakeHttp {
    /// Answer requests for `url` with `response`.
    pub fn respond(&self, url: &str, response: HttpResponse) {
        lock(&self.responses).insert(url.to_owned(), response);
    }

    /// Every request sent, oldest first.
    #[must_use]
    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.requests).clone()
    }
}

impl HttpClient for FakeHttp {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, PlatformError> {
        lock(&self.requests).push(request.clone());
        lock(&self.responses)
            .get(&request.url)
            .cloned()
            .ok_or_else(|| PlatformError::Failed(format!("no fake response for {}", request.url)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::platform::HttpMethod;

    #[tokio::test]
    async fn the_fakes_report_what_the_test_set() {
        let fakes = FakePlatform::new();
        let platform = fakes.platform();

        fakes.modifiers.set(Some(vec![Modifier::Shift]));
        assert_eq!(platform.modifiers.held().await, Some(vec![Modifier::Shift]));

        let mut locked = platform.lock.locked();
        fakes.lock.set(true);
        assert!(*locked.borrow_and_update());

        fakes.clipboard.copy("https://example.com/");
        assert_eq!(
            platform.clipboard.read().await,
            Ok(Some("https://example.com/".to_owned()))
        );
    }

    #[tokio::test]
    async fn the_fakes_record_what_the_service_asked() {
        let fakes = FakePlatform::new();
        let platform = fakes.platform();

        let command = LaunchCommand {
            argv: vec!["firefox".into(), "https://example.com/".into()],
            ..LaunchCommand::default()
        };
        let pid = platform.launcher.launch(&command).await.expect("launched");
        assert_eq!(pid, FIRST_FAKE_PID + 1);
        assert_eq!(fakes.launcher.launched(), vec![command]);

        let id = platform
            .notifier
            .notify(&Notification::default())
            .await
            .expect("shown");
        assert_eq!(id, 1);
        assert_eq!(fakes.notifier.shown().len(), 1);
    }

    #[tokio::test]
    async fn binding_a_shortcut_replaces_the_previous_trigger() {
        let fakes = FakeShortcuts::default();
        fakes.bind("toggle-menu", "Meta+W").await.expect("bound");
        fakes.bind("toggle-menu", "").await.expect("cleared");
        assert_eq!(
            fakes.bindings().await.expect("listed"),
            vec![BoundShortcut {
                action: "toggle-menu".into(),
                trigger: None,
            }]
        );
    }

    #[test]
    fn http_answers_only_known_urls() {
        let fakes = FakeHttp::default();
        let request = HttpRequest {
            method: HttpMethod::Head,
            url: "https://bit.ly/x".into(),
            timeout: Duration::from_secs(1),
            read_body: false,
        };
        assert!(fakes.send(&request).is_err());
        fakes.respond(
            "https://bit.ly/x",
            HttpResponse {
                status: 301,
                location: Some("https://example.com/".into()),
                body: None,
            },
        );
        assert_eq!(fakes.send(&request).expect("answered").status, 301);
        assert_eq!(fakes.requests().len(), 2);
    }
}
