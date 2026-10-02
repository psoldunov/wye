//! The picker broker: `PreviewPicker`, `PickerChose`, `PickerCancelled`,
//! `PickerAction` (PICK-01 to PICK-33, PIPE-13, PKS-06, PKS-07, IN-06).
//!
//! A link that resolves to the Picker becomes a `PickerRequest`, sent with
//! `PickerHost1.ShowPicker` to the UI host (bus-activating it). The UI
//! answers with one of the members here, naming the request. A new link
//! supersedes the pending one (PICK-27). When the UI cannot be reached the
//! link opens through the stand-in and a notification says so.

mod activation;
mod choice;
mod frontend;
pub(crate) mod host;
mod pending;
pub(crate) mod ready;
mod request;

use tokio::task::JoinHandle;
use url::Url;
use wye_api::Error;
use wye_api::actions::{PickerAction, Window};
use wye_core::picker::SourceLabel;
use wye_core::{Modifiers, SourceApp, Target};
use wye_desktop::Locale;

use self::frontend::Host;
use self::pending::{Offer, Pending, PendingLink, Registry};
use super::link::{self, Activation, PickerNeeded, Snapshot};
use super::{Caller, Dict, Result};
use crate::context::ServiceContext;
use crate::platform::{Notification, Platform};

/// The sample link Preview Picker shows (PKS-06).
const PREVIEW_LINK: &str = "https://example.com/articles/preview?from=wye";

/// State this topic keeps: the request the picker shows.
#[derive(Debug, Default)]
pub struct State {
    pending: Registry,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }
}

/// Show the picker for a link (PIPE-13): the request is built and made the
/// pending one here, and `ShowPicker` goes to the UI host in the
/// background, so `OpenLink` answers once the link is decided, not after a
/// cold `wye-ui` started. Falls back to the stand-in, with a notification,
/// when the UI host cannot show it.
pub(crate) async fn show_link(
    ctx: &ServiceContext,
    needed: PickerNeeded,
    activation: Activation,
) -> Result<()> {
    let placement = ctx.platform().pointer.pointer().await;
    let locale = locale(ctx)?;
    let for_request = needed.clone();
    let activation_token = activation
        .token
        .clone()
        .or_else(|| activation.startup_id.clone());
    let text = link::with_snapshot(ctx, move |snapshot| {
        let input = request::Input {
            config: snapshot.pipeline.config(),
            catalog: &catalog(snapshot, &locale),
            url: &for_request.resolution.url,
            source: source_label(snapshot, &locale, &for_request.request.source),
            held: for_request.request.held,
            placement,
            preview: false,
            activation_token,
        };
        encode(&input)
    })
    .await??;
    let (text, offered) = text;
    let (id, superseded) = ctx.picker().pending.open(
        Some(PendingLink { needed, activation }),
        offered,
        text.clone(),
    );
    if let Some(old) = &superseded {
        tracing::info!(
            old = old.id,
            new = id,
            "a new link replaces the pending one"
        );
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = deliver(&ctx, id, &text, superseded).await {
            tracing::warn!(%error, "cannot open the link without the picker");
        }
    });
    Ok(())
}

/// Send request `id` to the UI host; when it cannot show it, close it there
/// and open the link through the stand-in. `superseded` is the request `id`
/// replaced (PICK-27).
async fn deliver(
    ctx: &ServiceContext,
    id: String,
    text: &str,
    superseded: Option<Pending>,
) -> Result<()> {
    let error = match host::show_picker(ctx, &id, text).await {
        Ok(host) => {
            shown(ctx, &id, host, superseded);
            return Ok(());
        }
        Err(error) => error,
    };
    tracing::warn!(%error, "cannot show the picker");
    // A UI that answers late must not show a picker for a link the
    // stand-in already opened (PIPE-13). In the background: an unreachable
    // UI would hold the link for another deadline.
    let closing = ctx.clone();
    let closing_id = id.clone();
    tokio::spawn(async move {
        if let Err(error) = host::close_picker(&closing, &closing_id).await {
            tracing::debug!(%error, "cannot close the picker that did not show");
        }
    });
    // Only this request falls back; a newer one already replaced it.
    match ctx
        .picker()
        .pending
        .take(&id)
        .and_then(|pending| pending.link)
    {
        Some(pending) => stand_in(ctx, pending, &error).await,
        None => Ok(()),
    }
}

/// Redisplay the still-pending request when Shell disappears (PICK-27),
/// on the next host `advanced.frontend` names (ADV-12). `deliver` handles
/// the stand-in if no host is available.
pub(super) async fn shell_left(ctx: &ServiceContext) {
    // PKS-07: the Shell switches its extensions off while the screen is
    // locked; the lock closes the picker and holds its link instead.
    if *ctx.platform().lock.locked().borrow() {
        return;
    }
    let Some(pending) = ctx.picker().pending.current() else {
        return;
    };
    // Only a request the Shell shows; one on its way there finds the Shell
    // gone and goes on by itself.
    if pending.shown_on != Some(Host::Shell) {
        return;
    }
    if let Err(error) = deliver(ctx, pending.id, &pending.request, None).await {
        tracing::warn!(%error, "cannot hand the picker to the next host");
    }
}

/// ADV-12, PICK-27: record that `host` shows request `id`, and close what a
/// newer request left on another host: the request `id` replaced, or `id`
/// itself when a newer one replaced it on the way. A host that shows the
/// newer request has replaced the older one already, so it is left alone.
fn shown(ctx: &ServiceContext, id: &str, host: Host, superseded: Option<Pending>) {
    let pending = &ctx.picker().pending;
    if pending.mark_shown(id, host) {
        if let Some(old) = superseded.filter(|old| old.shown_on != Some(host)) {
            close_in_background(ctx, old.id, move |other| other != host);
        }
        return;
    }
    let newer = pending.current().and_then(|current| current.shown_on);
    if newer.is_some_and(|newer| newer != host) {
        close_in_background(ctx, id.to_owned(), move |other| other == host);
    }
}

/// `ClosePicker(id)` on the running hosts `which` accepts, in the
/// background.
fn close_in_background(
    ctx: &ServiceContext,
    id: String,
    which: impl Fn(Host) -> bool + Send + 'static,
) {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = host::close_running(&ctx, &id, which).await {
            tracing::debug!(%error, "cannot close a replaced picker");
        }
    });
}

/// The stand-in opens the link and a notification says why (PIPE-13).
async fn stand_in(ctx: &ServiceContext, pending: PendingLink, error: &Error) -> Result<()> {
    let notification = Notification {
        summary: "Wye couldn't show the picker".to_owned(),
        body: format!(
            "The link opened in your likeliest browser instead.\n{}",
            pending.needed.resolution.url
        ),
        ..Notification::default()
    };
    if let Err(notify_error) = ctx.platform().notifier.notify(&notification).await {
        tracing::warn!(%notify_error, %error, "cannot say the picker is unavailable");
    }
    super::picker_fallback::open_without_picker(ctx, pending.needed, &pending.activation).await
}

/// `dev.soldunov.wye1.PreviewPicker` (IN-06, PKS-06): the picker with a
/// sample link; choosing opens nothing.
pub async fn preview_picker(ctx: &ServiceContext) -> Result<()> {
    let url = Url::parse(PREVIEW_LINK).map_err(|error| Error::failed(error.to_string()))?;
    let placement = ctx.platform().pointer.pointer().await;
    let locale = locale(ctx)?;
    let text = link::with_snapshot(ctx, move |snapshot| {
        let input = request::Input {
            config: snapshot.pipeline.config(),
            catalog: &catalog(snapshot, &locale),
            url: &url,
            source: None,
            held: Modifiers::NONE,
            placement,
            preview: true,
            activation_token: None,
        };
        encode(&input)
    })
    .await??;
    let (text, offered) = text;
    let (id, superseded) = ctx.picker().pending.open(None, offered, text.clone());
    match host::show_picker(ctx, &id, &text).await {
        Ok(host) => {
            shown(ctx, &id, host, superseded);
            Ok(())
        }
        Err(error) => {
            ctx.picker().pending.take(&id);
            Err(error)
        }
    }
}

/// `dev.soldunov.wye1.PickerChose` (PIPE-13, PICK-20, PICK-21, PICK-29,
/// PICK-32, PICK-33).
pub async fn picker_chose(
    ctx: &ServiceContext,
    _caller: &Caller,
    request_id: &str,
    target: &str,
    options: &Dict,
) -> Result<()> {
    // Validated before the request is taken, so bad input loses nothing.
    let choice = choice::parse(target, options)?;
    match ctx.picker().pending.offers(request_id, &choice.target) {
        Offer::Offered => {}
        Offer::NotOffered => {
            return Err(Error::invalid_args(format!(
                "target: {} is not one the picker showed",
                choice.target
            )));
        }
        Offer::NotPending => return Err(not_pending(request_id)),
    }
    let Some(link) = take(ctx, request_id)?.link else {
        // PKS-06: a preview opens nothing.
        return Ok(());
    };
    let activation = choice.token.map_or(link.activation, |token| Activation {
        token: Some(token),
        startup_id: None,
    });
    let needed = link.needed;
    let chosen = choice.chosen;
    // PIPE-14: the matched rule's script runs on the chosen link.
    let hooks = link::hooks::LinkHooks::scripts_only(ctx);
    let plan = link::with_snapshot(ctx, move |snapshot| {
        link::plan_with(
            snapshot,
            &needed.resolution,
            &needed.request,
            Some(chosen),
            hooks.hooks(),
        )
    })
    .await?;
    // The plan carries its history entry; `open_plan` records it once the
    // browser started (PIPE-16).
    link::open_plan(ctx, plan, &activation).await
}

/// `dev.soldunov.wye1.PickerCancelled` (PICK-23): the link is dropped.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function"
)]
pub async fn picker_cancelled(
    ctx: &ServiceContext,
    _caller: &Caller,
    request_id: &str,
) -> Result<()> {
    take(ctx, request_id).map(|pending| {
        tracing::info!(id = pending.id, "the picker was cancelled");
    })
}

/// `dev.soldunov.wye1.PickerAction`: `copy-link` (KEY-22) or `create-rule`
/// (PICK-31). Either ends the request without opening the link.
pub async fn picker_action(
    ctx: &ServiceContext,
    _caller: &Caller,
    request_id: &str,
    action: &str,
) -> Result<()> {
    let action: PickerAction = action
        .parse()
        .map_err(|error: wye_api::UnknownValue| Error::invalid_args(error.to_string()))?;
    let pending = take(ctx, request_id)?;
    let url = pending.link.as_ref().map_or_else(
        || PREVIEW_LINK.to_owned(),
        |link| link.needed.resolution.url.to_string(),
    );
    match action {
        PickerAction::CopyLink => ctx
            .platform()
            .clipboard
            .write(&url)
            .await
            .map_err(|error| Error::failed(error.to_string())),
        PickerAction::CreateRule => {
            let prefill = rule_prefill(pending.link.as_ref(), &url);
            host::show_window(ctx, Window::RuleEditor, &prefill).await
        }
    }
}

/// PICK-31: the rule editor's prefill, a Domain matcher for the link's host
/// and the source app when known: `{"domain": …, "sourceApp": …}`.
fn rule_prefill(link: Option<&PendingLink>, url: &str) -> String {
    let host = Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    let source = link
        .and_then(|link| link.needed.request.source.desktop_id.as_ref())
        .map(ToString::to_string);
    serde_json::json!({ "domain": host, "sourceApp": source }).to_string()
}

fn take(ctx: &ServiceContext, request_id: &str) -> Result<Pending> {
    ctx.picker()
        .pending
        .take(request_id)
        .ok_or_else(|| not_pending(request_id))
}

fn not_pending(request_id: &str) -> Error {
    Error::NotFound(format!("no picker request {request_id:?} is pending"))
}

/// The request's JSON, and every target `PickerChose` may answer it with
/// ([`request::offered`]).
fn encode(input: &request::Input<'_>) -> Result<(String, Vec<Target>)> {
    Ok((
        wye_api::json::encode(&request::build(input))?,
        request::offered(input),
    ))
}

/// Background work of this topic, started with the service: keep the UI
/// host started while a route can end on the picker (decision 2), and
/// close the picker when the screen locks.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![
        tokio::spawn(ready::keep_ready(ctx.clone())),
        tokio::spawn(ready::watch_shell(ctx.clone())),
        tokio::spawn(close_on_lock(ctx.clone())),
    ]
}

/// PKS-07: a picker showing when the screen locks closes, and its link
/// waits for the unlock instead of being lost.
async fn close_on_lock(ctx: ServiceContext) {
    let mut locked = ctx.platform().lock.locked();
    while locked.changed().await.is_ok() {
        if !*locked.borrow_and_update() {
            continue;
        }
        let Some(pending) = ctx.picker().pending.take_current() else {
            continue;
        };
        if let Err(error) = host::close_picker(&ctx, &pending.id).await {
            tracing::warn!(%error, "cannot close the picker");
        }
        if let Some(link) = pending.link {
            tracing::info!("screen locked; the picker's link waits for the unlock");
            link::hold_for_unlock(&ctx, link.needed, link.activation);
        }
    }
}

/// The session's message locale, for app names (DISC-03).
fn locale(ctx: &ServiceContext) -> Result<Locale> {
    Ok(ctx.environment()?.xdg.locale.clone())
}

fn catalog(snapshot: &Snapshot, locale: &Locale) -> wye_core::target_menu::TargetCatalog {
    snapshot.inventory.catalog(locale, &[])
}

/// The source app as the URL line shows it (PICK-09).
fn source_label(snapshot: &Snapshot, locale: &Locale, source: &SourceApp) -> Option<SourceLabel> {
    let installed = source
        .desktop_id
        .as_ref()
        .and_then(|id| snapshot.inventory.get(id));
    match (installed, &source.executable) {
        (Some(app), _) => Some(SourceLabel {
            name: app.display_name(locale),
            icon: app.entry.icon.clone(),
        }),
        (None, Some(executable)) => Some(SourceLabel {
            name: executable.clone(),
            icon: None,
        }),
        (None, None) => None,
    }
}
