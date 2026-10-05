//! Internal state: the `Status` property and `UpdateUiState` (SET-08,
//! BLK-09, RUL-19, ONB-06, ONB-10, ONB-11, KEY-06 capability).
//!
//! The state file (`$XDG_STATE_HOME/wye/state.toml`,
//! [`wye_desktop::State`]) is shared with the CLI, so it is read when needed
//! rather than cached, and every change loads, edits and saves it whole
//! under the file lock ([`wye_desktop::StateLock`]). An unreadable file is
//! reported, never replaced.

mod ui;

use serde_json::Value;
use wye_api::status::{Capabilities, ConfigStatus, Status};
use wye_api::{Error, json};

use super::Result;
use crate::bus::Property;
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;
use wye_core::config::HeldKeys;

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    /// Serialises read-modify-write cycles of the state file.
    writing: tokio::sync::Mutex<()>,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }
}

/// The state file; the empty state when it is missing.
///
/// # Errors
///
/// `Failed` when there is no home directory or the file cannot be read.
pub(crate) async fn load(ctx: &ServiceContext) -> Result<wye_desktop::State> {
    let path = ctx.environment()?.state.clone();
    blocking(move || wye_desktop::State::load(&path))
        .await?
        .map_err(|error| Error::failed(error.to_string()))
}

/// Load the state, change it with `change`, save it when it changed and
/// announce `Status`. Returns the saved state. The file lock is held from
/// the load to the save, so the CLI's writes cannot slip in between.
///
/// # Errors
///
/// `Failed` when the file cannot be locked, read or written; an unreadable
/// file is reported and left as it is. The error from `change`.
pub(crate) async fn update(
    ctx: &ServiceContext,
    change: impl FnOnce(&wye_desktop::State) -> Result<wye_desktop::State>,
) -> Result<wye_desktop::State> {
    let writing = ctx.state().writing.lock().await;
    let path = ctx.environment()?.state.clone();
    let (lock, before) = blocking(move || {
        let lock = wye_desktop::StateLock::acquire(&path)?;
        let before = lock.load()?;
        Ok::<_, wye_desktop::StateError>((lock, before))
    })
    .await?
    .map_err(|error| Error::failed(error.to_string()))?;
    let after = change(&before)?;
    if after == before {
        return Ok(after);
    }
    let saved = after.clone();
    blocking(move || lock.save(&saved))
        .await?
        .map_err(|error| Error::failed(error.to_string()))?;
    drop(writing);
    super::config::effects::changed(ctx, Property::Status);
    Ok(after)
}

/// The `Status` property as JSON.
pub async fn status_json(ctx: &ServiceContext) -> Result<String> {
    json::encode(&status(ctx).await)
}

/// Everything `Status` reports. Parts that cannot be read are left at their
/// defaults and logged; the property never fails for them.
pub(crate) async fn status(ctx: &ServiceContext) -> Status {
    let platform = ctx.platform();
    let state = load(ctx).await.unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read the state file");
        wye_desktop::State::default()
    });
    let held_keys = super::config::current(ctx)
        .await
        .map_or(HeldKeys::Auto, |current| current.config.advanced.held_keys);
    Status {
        default_browser: super::default_browser::status(ctx, &state).await,
        config: config_status(ctx).await,
        capabilities: capabilities(&platform, held_keys),
        locked: *platform.lock.locked().borrow(),
        login_managed: ctx.login_managed().is_some(),
        login_managed_on: ctx.login_managed() == Some(true),
        ui_state: ui::from_state(&state),
    }
}

async fn config_status(ctx: &ServiceContext) -> ConfigStatus {
    match super::config::current(ctx).await {
        Ok(current) => ConfigStatus {
            path: current.environment.config.display().to_string(),
            writable: current.writable,
            lossless: current.lossless,
            warnings: current.warning_lines(),
            error: current.error.clone(),
        },
        Err(error) => ConfigStatus {
            error: Some(error.to_string()),
            ..ConfigStatus::default()
        },
    }
}

/// What the session supports, from the platform integrations (KEY-06,
/// DLG-ABT-02).
/// `advanced.held-keys = "off"` makes held keys unavailable (KEY-06).
fn capabilities(platform: &Platform, held_keys: HeldKeys) -> Capabilities {
    let clipboard = platform.clipboard.capabilities();
    let probe = platform
        .modifiers
        .mechanism()
        .filter(|_| held_keys == HeldKeys::Auto);
    Capabilities {
        held_keys: probe.map(str::to_owned),
        pointer: platform.pointer.mechanism().map(str::to_owned),
        source_app_fallbacks: platform
            .focus
            .mechanism()
            .map(str::to_owned)
            .into_iter()
            .collect(),
        clipboard_read: clipboard.read.map(str::to_owned),
        clipboard_watch: clipboard.watch.map(str::to_owned),
        clipboard_write: clipboard.write.map(str::to_owned),
        global_shortcuts: platform.shortcuts.mechanism().map(str::to_owned),
        lock_detection: platform.lock.mechanism().map(str::to_owned),
        // The held-key probe is a layer surface, so it proves the compositor
        // offers `zwlr_layer_shell_v1`; with the probe off this reads false.
        layer_shell: probe == Some(crate::platform::modifiers::wayland::MECHANISM),
    }
}

/// `dev.soldunov.wye1.UpdateUiState`: merge patch of
/// [`wye_api::status::UiState`] (SET-08, BLK-09, RUL-19, ONB-06).
pub async fn update_ui_state(ctx: &ServiceContext, merge_patch: &str) -> Result<()> {
    let patch: Value = serde_json::from_str(merge_patch)
        .map_err(|error| Error::invalid_args(format!("merge patch: {error}")))?;
    update(ctx, |state| ui::patched(state, &patch))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::link::Environment;
    use crate::platform::fake::{FAKE, FakePlatform};

    /// A context over `platform` that reads a temporary home.
    fn context(platform: Platform, home: &std::path::Path) -> ServiceContext {
        let ctx = ServiceContext::new(platform);
        let home = home.to_owned();
        let environment = Environment::from_lookup(move |name| {
            (name == "HOME").then(|| std::ffi::OsString::from(home.clone()))
        })
        .expect("home");
        ctx.set_environment(environment);
        ctx
    }

    /// An outside writer (`wye default set`) has loaded the state under the
    /// file lock when the service updates (adoption recording the browsers
    /// it offered): the service waits for the writer's save, then keeps its
    /// field. Without the lock the service would save first and the writer's
    /// stale copy would drop `seen-browsers`.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_waits_for_an_outside_writer() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(Platform::unavailable(), home.path());
        let path = ctx.environment().expect("home").state.clone();
        let outside = wye_desktop::StateLock::acquire(&path).expect("locked");
        let loaded = outside.load().expect("loads");
        let seen = vec![wye_core::DesktopId::new("firefox.desktop").expect("ID")];
        let writer = {
            let ctx = ctx.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                update(&ctx, |state| {
                    Ok(wye_desktop::State {
                        seen_browsers: Some(seen),
                        ..state.clone()
                    })
                })
                .await
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let previous = wye_core::DesktopId::new("chromium.desktop").expect("ID");
        outside
            .save(&wye_desktop::State {
                previous_default_browser: Some(previous.clone()),
                ..loaded
            })
            .expect("saved");
        drop(outside);
        writer.await.expect("joined").expect("updated");
        let state = wye_desktop::State::load(&path).expect("loads");
        assert_eq!(state.previous_default_browser, Some(previous));
        assert_eq!(state.seen_browsers, Some(seen));
    }

    /// The lock must span both `blocking` hops of `update` (load, then save).
    /// An outside writer starts to lock while `change` runs, between the two
    /// hops; if `update` released the lock there, the writer would load the
    /// old state, and this test would see the change missing.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_lock_is_held_between_the_load_and_the_save() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(Platform::unavailable(), home.path());
        let path = ctx.environment().expect("home").state.clone();
        let (started, start) = std::sync::mpsc::channel::<()>();
        let outside = std::thread::spawn(move || {
            start.recv().expect("signalled");
            let lock = wye_desktop::StateLock::acquire(&path).expect("locked");
            lock.load().expect("loads")
        });
        update(&ctx, |state| {
            started.send(()).expect("signalled");
            std::thread::sleep(std::time::Duration::from_millis(100));
            Ok(wye_desktop::State {
                onboarding_done: true,
                ..state.clone()
            })
        })
        .await
        .expect("updated");
        let seen = outside.join().expect("joined");
        assert!(seen.onboarding_done, "the outside writer saw the old state");
    }

    #[tokio::test]
    async fn an_unreadable_state_file_is_reported_not_replaced() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(Platform::unavailable(), home.path());
        let path = ctx.environment().expect("home").state.clone();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(&path, "previous-default-browser = 3\n").expect("written");
        let result = update(&ctx, |state| {
            Ok(wye_desktop::State {
                onboarding_done: true,
                ..state.clone()
            })
        })
        .await;
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(&path).expect("read"),
            b"previous-default-browser = 3\n"
        );
    }

    #[tokio::test]
    async fn status_reports_the_platforms_capabilities() {
        let fakes = FakePlatform::new();
        fakes.lock.set(true);
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(fakes.platform(), home.path());
        let status: Status =
            json::decode("status", &status_json(&ctx).await.expect("encodes")).expect("decodes");
        assert!(status.locked);
        assert_eq!(status.capabilities.held_keys.as_deref(), Some(FAKE));
        assert_eq!(
            status.capabilities.source_app_fallbacks,
            vec![FAKE.to_owned()]
        );
    }

    #[tokio::test]
    async fn an_unavailable_platform_reports_nothing() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(Platform::unavailable(), home.path());
        let status: Status =
            json::decode("status", &status_json(&ctx).await.expect("encodes")).expect("decodes");
        assert_eq!(status.capabilities, Capabilities::default());
    }
}
