//! Internal state: the `Status` property and `UpdateUiState` (SET-08,
//! BLK-09, RUL-19, ONB-06, KEY-06 capability).

use wye_api::Error;
use wye_api::json;
use wye_api::status::{Capabilities, Status};

use super::Result;
use crate::context::ServiceContext;
use crate::platform::Platform;

/// State this topic keeps. Empty until the topic is implemented.
#[derive(Debug, Default)]
pub struct State;

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self
    }
}

/// The `Status` property as JSON.
///
/// For now only the capabilities and the lock state are real; the rest are
/// defaults.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn status_json(ctx: &ServiceContext) -> Result<String> {
    let platform = ctx.platform();
    let status = Status {
        capabilities: capabilities(platform),
        locked: *platform.lock.locked().borrow(),
        ..Status::default()
    };
    json::encode(&status)
}

/// What the session supports, from the platform integrations (KEY-06,
/// DLG-ABT-02).
fn capabilities(platform: &Platform) -> Capabilities {
    let clipboard = platform.clipboard.capabilities();
    Capabilities {
        held_keys: platform.modifiers.mechanism().map(str::to_owned),
        pointer: platform.pointer.mechanism().map(str::to_owned),
        source_app_fallbacks: platform
            .focus
            .mechanism()
            .map(str::to_owned)
            .into_iter()
            .collect(),
        clipboard_read: clipboard.read.map(str::to_owned),
        clipboard_watch: clipboard.watch.map(str::to_owned),
        global_shortcuts: platform.shortcuts.mechanism().map(str::to_owned),
        lock_detection: platform.lock.mechanism().map(str::to_owned),
        layer_shell: false,
    }
}

/// `dev.soldunov.wye1.UpdateUiState`: merge patch of
/// [`wye_api::status::UiState`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn update_ui_state(_ctx: &ServiceContext, _merge_patch: &str) -> Result<()> {
    Err(Error::not_implemented("UpdateUiState"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::fake::{FAKE, FakePlatform};

    #[tokio::test]
    async fn status_reports_the_platforms_capabilities() {
        let fakes = FakePlatform::new();
        fakes.lock.set(true);
        let ctx = ServiceContext::new(fakes.platform());
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
        let ctx = ServiceContext::new(Platform::unavailable());
        let status: Status =
            json::decode("status", &status_json(&ctx).await.expect("encodes")).expect("decodes");
        assert_eq!(status.capabilities, Capabilities::default());
    }
}
