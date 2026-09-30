//! The tray: the `Tray` property, `ActivateTrayItem`, the
//! `StatusNotifierItem` that is Wye's tray on every desktop, and
//! `RegisterTray` for external tray hosts (TRAY-01 to TRAY-18, ONB-11,
//! decision 8).
//!
//! One model, `wye_core::tray::TrayMenu`, built from the configuration, the
//! installed apps, the clipboard, the default-browser state and the history,
//! feeds every surface: the SNI tray, the `wye-ui` popup and any external
//! host. They send back the chosen item's ID; [`activate_tray_item`] does
//! the rest.

mod action;
mod dto;
mod hosts;
mod present;

use std::collections::HashMap;
use std::str::FromStr as _;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::task::JoinHandle;
use wye_api::Error;
use wye_api::actions::TrayHost;
use wye_api::history::History;
use wye_api::json;
use wye_api::tray::TrayMenu;
use wye_core::tray::{self as model, RecentLink, TrayStatus};

use self::action::{HELP_URL, RECENT_REOPEN, TrayAction};
use self::hosts::Hosts;
use super::{Caller, Result};
use crate::context::ServiceContext;
use crate::platform::Platform;

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    /// External tray hosts that registered (decision 8).
    hosts: Hosts,
    /// The `StatusNotifierItem` is shown.
    sni_shown: AtomicBool,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }

    fn set_sni_shown(&self, shown: bool) {
        self.sni_shown.store(shown, Ordering::Relaxed);
    }
}

/// The tray task: shows the `StatusNotifierItem` unless an external tray
/// host is registered.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![tokio::spawn(present::present(ctx.clone()))]
}

/// TRAY-05: whether the user can see Wye's tray icon now, in a registered
/// tray host or as the `StatusNotifierItem`. `Activate` opens Settings when
/// not, so the user can always get back in.
pub(crate) async fn icon_on_screen(ctx: &ServiceContext) -> bool {
    let visible = match super::config::current(ctx).await {
        Ok(current) => current.config.general.show_tray_icon,
        Err(_) => return false,
    };
    visible && (ctx.tray().hosts.any() || ctx.tray().sni_shown.load(Ordering::Relaxed))
}

/// The `Tray` property as JSON.
///
/// # Errors
///
/// When the configuration or the installed apps cannot be read.
pub async fn tray_json(ctx: &ServiceContext) -> Result<String> {
    json::encode(&build(ctx).await?)
}

/// The menu as it is now (TRAY-02 to TRAY-18).
async fn build(ctx: &ServiceContext) -> Result<TrayMenu> {
    let environment = ctx.environment()?;
    let config = super::config::current(ctx).await?;
    let inventory = super::inventory::current(ctx).await?;
    let catalog = inventory.inventory.catalog(&environment.xdg.locale, &[]);
    let status = TrayStatus {
        clipboard_has_url: clipboard_has_url(ctx).await,
        wye_is_default: wye_is_default(ctx).await,
        recent: recent(ctx).await,
    };
    let menu = model::TrayMenu::build(&config.config, &catalog, &status);
    Ok(dto::menu(&menu))
}

/// TRAY-10: an unreadable clipboard counts as holding no URL.
async fn clipboard_has_url(ctx: &ServiceContext) -> bool {
    super::clipboard::clipboard_has_url(ctx)
        .await
        .inspect_err(|error| tracing::debug!(%error, "clipboard unknown for the tray"))
        .unwrap_or(false)
}

/// TRAY-18: only the registration matters here, not the remembered
/// browsers.
async fn wye_is_default(ctx: &ServiceContext) -> bool {
    super::default_browser::status(ctx, &wye_desktop::State::default())
        .await
        .is_default
}

/// TRAY-15: the newest history entries, newest first.
async fn recent(ctx: &ServiceContext) -> Vec<RecentLink> {
    let history = match super::history::get_history(ctx).await {
        Ok(text) => json::decode::<History>("history", &text),
        Err(error) => Err(error),
    };
    let mut entries = match history {
        Ok(history) if history.enabled => history.entries,
        Ok(_) => return Vec::new(),
        Err(error) => {
            tracing::debug!(%error, "no recent links for the tray");
            return Vec::new();
        }
    };
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.time));
    entries
        .into_iter()
        .take(model::RECENT_LIMIT)
        .map(|entry| RecentLink {
            id: entry.id.to_string(),
            url: entry.final_url,
        })
        .collect()
}

/// `dev.soldunov.wye1.RegisterTray` ([`wye_api::actions::TrayHost`]): the
/// caller shows the tray itself, so the `StatusNotifierItem` goes away until
/// the caller unregisters as often as it registered, or its connection goes
/// away.
///
/// # Errors
///
/// `InvalidArgs` for an unknown kind or a caller without a bus name;
/// `Failed` when the service is not on a bus.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; the watch runs in its own task"
)]
pub async fn register_tray(ctx: &ServiceContext, caller: &Caller, kind: &str) -> Result<()> {
    let kind = TrayHost::from_str(kind).map_err(|error| Error::invalid_args(error.to_string()))?;
    let sender = caller
        .sender
        .clone()
        .ok_or_else(|| Error::invalid_args("RegisterTray needs a bus connection"))?;
    let connection = ctx
        .connection()
        .cloned()
        .ok_or_else(|| Error::failed("the service is not on a bus"))?;
    tracing::info!(host = sender, kind = kind.as_str(), "tray host registered");
    if !ctx.tray().hosts.insert(&sender) {
        // Watched already: another host instance on the same connection.
        return Ok(());
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        hosts::until_gone(&connection, &sender).await;
        tracing::info!(host = sender, "tray host left");
        ctx.tray().hosts.forget(&sender);
    });
    Ok(())
}

/// `dev.soldunov.wye1.UnregisterTray`: one of the caller's registrations
/// ends (a host instance was removed or disabled). When it was the last,
/// the `StatusNotifierItem` comes back at once.
/// Unregistering a caller that never registered changes nothing.
///
/// # Errors
///
/// `InvalidArgs` for a caller without a bus name.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await"
)]
pub async fn unregister_tray(ctx: &ServiceContext, caller: &Caller) -> Result<()> {
    let sender = caller
        .sender
        .as_deref()
        .ok_or_else(|| Error::invalid_args("UnregisterTray needs a bus connection"))?;
    if ctx.tray().hosts.remove(sender) {
        tracing::info!(host = sender, "tray host unregistered");
    } else {
        tracing::debug!(
            host = sender,
            "a registration ended; the host still shows the tray"
        );
    }
    Ok(())
}

/// `dev.soldunov.wye1.ActivateTrayItem`: carry out the tray item `id`
/// (01-tray-menu.md menu layout).
///
/// # Errors
///
/// `InvalidArgs` for an ID that names no choosable item; otherwise the
/// error of the call the item makes.
pub async fn activate_tray_item(ctx: &ServiceContext, id: &str) -> Result<()> {
    let nobody = Caller { sender: None };
    match TrayAction::parse(id)? {
        TrayAction::MakeDefault => super::default_browser::make_default(ctx).await,
        TrayAction::OpenClipboard => super::clipboard::open_clipboard(ctx, &nobody, false).await,
        TrayAction::Primary(id) => set_primary(ctx, &id).await,
        TrayAction::Show(window) => super::windows::show_window(ctx, window.as_str(), "").await,
        TrayAction::Recent(entry) => {
            super::history::reopen_history_entry(ctx, entry, RECENT_REOPEN.as_str()).await
        }
        TrayAction::Rescan => super::inventory::rescan(ctx).await,
        TrayAction::Help => super::link::open_link(ctx, &nobody, HELP_URL, &HashMap::new()).await,
        TrayAction::Quit => super::windows::quit(ctx).await,
    }
}

/// TRAY-11: the radio item's target becomes the primary browser.
async fn set_primary(ctx: &ServiceContext, id: &str) -> Result<()> {
    let menu = {
        let environment = ctx.environment()?;
        let config = super::config::current(ctx).await?;
        let inventory = super::inventory::current(ctx).await?;
        let catalog = inventory.inventory.catalog(&environment.xdg.locale, &[]);
        let status = TrayStatus {
            wye_is_default: true,
            ..TrayStatus::default()
        };
        model::TrayMenu::build(&config.config, &catalog, &status)
    };
    let target = menu
        .find(id)
        .and_then(|item| item.target.clone())
        .ok_or_else(|| Error::NotFound(format!("no primary-browser item {id:?}")))?;
    super::config::set_primary(ctx, &json::encode(&target)?).await
}
