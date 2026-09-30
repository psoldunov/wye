//! The clipboard: `OpenClipboard`, `ClipboardHasUrl`, and the copy-time
//! rewrites (IN-02 to IN-04, TRAY-10, EXT-02 to EXT-15).

mod rewrite;

use std::collections::HashMap;

use tokio::task::JoinHandle;
use wye_api::{Error, context};
use wye_core::clipboard::clipboard_link;
use zbus::zvariant::{OwnedValue, Value};

use super::{Caller, Dict, Result};
use crate::context::ServiceContext;
use crate::platform::Platform;

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    rewrites: rewrite::Rewrites,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }
}

/// Background work: rewrite copied links (EXT-12).
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![tokio::spawn(rewrite::watch(ctx.clone()))]
}

/// `dev.soldunov.wye1.OpenClipboard` (IN-02 to IN-04): the link on the
/// clipboard through the full pipeline. The source app is unknown, and
/// `alternative` acts as if the alternative-browser key were held
/// (PIPE-06). Held keys are not probed: the keys of the shortcut that asked
/// are still down.
///
/// # Errors
///
/// `NotFound` when the clipboard holds no link, `Unavailable` when the
/// session cannot read it; the link path's errors otherwise.
pub async fn open_clipboard(
    ctx: &ServiceContext,
    caller: &Caller,
    alternative: bool,
) -> Result<()> {
    let url = link(ctx)
        .await?
        .ok_or_else(|| Error::NotFound("the clipboard holds no link".to_owned()))?;
    let force = if alternative {
        context::Force::Alternative
    } else {
        context::Force::None
    };
    let link_context = link_context(force)?;
    super::link::open_link(ctx, caller, url.as_str(), &link_context).await
}

/// `dev.soldunov.wye1.ClipboardHasUrl` (TRAY-10).
///
/// # Errors
///
/// `Unavailable` when the session cannot read the clipboard.
pub async fn clipboard_has_url(ctx: &ServiceContext) -> Result<bool> {
    Ok(link(ctx).await?.is_some())
}

/// The web link on the clipboard, if the text is one.
async fn link(ctx: &ServiceContext) -> Result<Option<url::Url>> {
    let text = ctx.platform().clipboard.read().await?;
    Ok(text.as_deref().and_then(clipboard_link))
}

/// An `OpenLink` context for a clipboard link: entry `clipboard`, held keys
/// known to be none, and `force`.
fn link_context(force: context::Force) -> Result<Dict> {
    let value = |value: Value<'_>| {
        OwnedValue::try_from(value)
            .map_err(|error| Error::failed(format!("cannot build the link context: {error}")))
    };
    let held: Vec<String> = Vec::new();
    Ok(HashMap::from([
        (
            context::ENTRY.to_owned(),
            value(Value::from(context::Entry::Clipboard.as_str()))?,
        ),
        (
            context::FORCE.to_owned(),
            value(Value::from(force.as_str()))?,
        ),
        (context::HELD_KNOWN.to_owned(), value(Value::from(true))?),
        (context::HELD.to_owned(), value(Value::from(held))?),
    ]))
}
