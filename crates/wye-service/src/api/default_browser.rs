//! Default-browser registration: `MakeDefault`, `StopBeingDefault`,
//! `KeepCurrentDefault`, the `defaultBrowser` part of `Status`, and
//! takeover detection (DEF-02, DEF-03, DEF-05, ONB-10, ONB-11).

mod change;
mod detect;
mod takeover;

use tokio::task::JoinHandle;
use wye_api::AppRef;
use wye_api::status::DefaultBrowserStatus;
use wye_core::DesktopId;
use wye_desktop::Inventory;

use super::Result;
use crate::bus::Property;
use crate::context::{ServiceContext, blocking};
use crate::platform::{Notification, Platform};

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    takeover: takeover::Watch,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }
}

/// Background work: answer the takeover notification's buttons.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![takeover::spawn(ctx)]
}

/// `dev.soldunov.wye1.MakeDefault` (DEF-02): Wye handles web links, and
/// remembers what it replaced (DEF-05).
pub async fn make_default(ctx: &ServiceContext) -> Result<()> {
    let xdg = ctx.environment()?.xdg.clone();
    let include_html = super::config::current(ctx)
        .await?
        .config
        .general
        .open_local_html;
    // DEF-05: an unreadable state file stops the call before anything
    // changes, so the replaced browser is never left unrecorded.
    super::state::load(ctx).await?;
    let made = blocking(move || change::make_default(&xdg, include_html)).await??;
    super::state::update(ctx, |state| {
        Ok(wye_desktop::State {
            previous_default_browser: made
                .previous
                .clone()
                .or_else(|| state.previous_default_browser.clone()),
            previous_kdeglobals_browser: match &made.kdeglobals {
                change::Kdeglobals::Replaced(previous) => previous.clone(),
                change::Kdeglobals::Untouched => state.previous_kdeglobals_browser.clone(),
            },
            kept_default: None,
            ..state.clone()
        })
    })
    .await?;
    takeover::withdraw(ctx).await;
    // DEF-03: look at the registration now rather than after the watcher's
    // quiet time, so a takeover right after this call is still noticed.
    check_takeover(ctx).await;
    Ok(())
}

/// `dev.soldunov.wye1.StopBeingDefault` (DEF-05): give links back to the
/// browser Wye replaced. Nothing changes when Wye is not the default.
pub async fn stop_being_default(ctx: &ServiceContext) -> Result<()> {
    let xdg = ctx.environment()?.xdg.clone();
    let include_html = super::config::current(ctx)
        .await?
        .config
        .general
        .open_local_html;
    let state = super::state::load(ctx).await?;
    let previous = state.previous_default_browser.clone();
    let kde = state.previous_kdeglobals_browser.clone();
    let restored = blocking(move || {
        change::stop_being_default(&xdg, previous.as_ref(), kde.as_deref(), include_html)
    })
    .await??;
    if let Some(restored) = restored {
        // The user chose this; it is not a takeover to warn about (ONB-11).
        super::state::update(ctx, |state| {
            Ok(wye_desktop::State {
                previous_default_browser: None,
                previous_kdeglobals_browser: None,
                kept_default: Some(restored.clone()),
                ..state.clone()
            })
        })
        .await?;
    }
    registration_changed(ctx);
    Ok(())
}

/// `dev.soldunov.wye1.KeepCurrentDefault` (ONB-10, ONB-11): keep the app
/// that handles links now and stop asking.
pub async fn keep_current_default(ctx: &ServiceContext) -> Result<()> {
    let xdg = ctx.environment()?.xdg.clone();
    let wye = change::wye_id()?;
    let found = blocking(move || detect::registration(&xdg, &wye)).await?;
    if found.is_default {
        return Ok(());
    }
    keep(ctx, found.current).await
}

/// DEF-07: `general.open-local-html` changed to `include_html`. While Wye is
/// the default browser, `mimeapps.list` follows it at once; otherwise
/// nothing changes until the next `MakeDefault`.
pub(crate) async fn html_setting_changed(ctx: &ServiceContext, include_html: bool) {
    let Ok(environment) = ctx.environment() else {
        return;
    };
    let xdg = environment.xdg.clone();
    let previous = if include_html {
        None
    } else {
        super::state::load(ctx)
            .await
            .ok()
            .and_then(|state| state.previous_default_browser)
    };
    let followed =
        blocking(move || change::follow_html(&xdg, include_html, previous.as_ref())).await;
    match followed {
        Ok(Ok(true)) => {
            tracing::info!(include_html, "HTML files follow the new setting");
            registration_changed(ctx);
        }
        Ok(Ok(false)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "cannot update the HTML file association"),
        Err(error) => tracing::warn!(%error, "cannot update the HTML file association"),
    }
}

/// Remember `app` as the default the user keeps (ONB-11).
async fn keep(ctx: &ServiceContext, app: Option<DesktopId>) -> Result<()> {
    super::state::update(ctx, |state| {
        Ok(wye_desktop::State {
            kept_default: app.clone(),
            ..state.clone()
        })
    })
    .await?;
    takeover::withdraw(ctx).await;
    registration_changed(ctx);
    Ok(())
}

/// The `defaultBrowser` part of `Status` (ONB-10, ONB-11).
pub(crate) async fn status(
    ctx: &ServiceContext,
    state: &wye_desktop::State,
) -> DefaultBrowserStatus {
    let Ok(environment) = ctx.environment() else {
        return DefaultBrowserStatus::default();
    };
    let Ok(wye) = change::wye_id() else {
        return DefaultBrowserStatus::default();
    };
    let xdg = environment.xdg.clone();
    let found = match blocking(move || detect::registration(&xdg, &wye)).await {
        Ok(found) => found,
        Err(error) => {
            tracing::warn!(%error, "cannot read the default browser");
            return DefaultBrowserStatus::default();
        }
    };
    let inventory = super::inventory::current(ctx).await.ok();
    let locale = &environment.xdg.locale;
    let app = |id: &DesktopId| {
        inventory
            .as_ref()
            .map_or_else(|| bare(id), |scan| app_ref(&scan.inventory, locale, id))
    };
    DefaultBrowserStatus {
        is_default: found.is_default,
        current: found.current.as_ref().map(&app),
        previous: state.previous_default_browser.as_ref().map(&app),
        kept_current: !found.is_default
            && found.current.is_some()
            && found.current == state.kept_default,
    }
}

/// DEF-03: `mimeapps.list` or `kdeglobals` changed. Tell clients, and
/// notify once when another app took over, unless the user keeps it.
pub(crate) async fn check_takeover(ctx: &ServiceContext) {
    let Ok(environment) = ctx.environment() else {
        return;
    };
    let Ok(wye) = change::wye_id() else {
        return;
    };
    let xdg = environment.xdg.clone();
    let Ok(found) = blocking(move || detect::registration(&xdg, &wye)).await else {
        return;
    };
    registration_changed(ctx);
    if found.is_default {
        // ONB-11: fixed, by Wye or by hand.
        takeover::withdraw(ctx).await;
    }
    let Some(takeover::TookOver { app: Some(app) }) = ctx.default_browser().takeover.saw(found)
    else {
        return;
    };
    let kept = super::state::load(ctx)
        .await
        .ok()
        .and_then(|state| state.kept_default);
    if kept.as_ref() == Some(&app) {
        return;
    }
    let name = match super::inventory::current(ctx).await {
        Ok(scan) => app_ref(&scan.inventory, &environment.xdg.locale, &app).name,
        Err(_) => app.to_string(),
    };
    let notification = Notification {
        // One takeover notification at a time: a newer one replaces it. The
        // old one is forgotten only once the new one is shown (`shown`).
        replaces: ctx.default_browser().takeover.on_screen(),
        ..takeover::notification(&name)
    };
    match ctx.platform().notifier.notify(&notification).await {
        Ok(id) => ctx.default_browser().takeover.shown(id, app),
        Err(error) => tracing::warn!(%error, "cannot show the takeover notification"),
    }
}

/// `Status` and the tray (TRAY-18) show the registration.
fn registration_changed(ctx: &ServiceContext) {
    super::config::effects::changed(ctx, Property::Status);
    super::config::effects::changed(ctx, Property::Tray);
}

/// An app as `Status` names it.
pub(crate) fn app_ref(
    inventory: &Inventory,
    locale: &wye_desktop::Locale,
    id: &DesktopId,
) -> AppRef {
    inventory.get(id).map_or_else(
        || bare(id),
        |app| AppRef {
            id: id.to_string(),
            name: app.display_name(locale),
            icon: app.entry.icon.clone(),
        },
    )
}

fn bare(id: &DesktopId) -> AppRef {
    AppRef {
        id: id.to_string(),
        name: id.as_str().trim_end_matches(".desktop").to_owned(),
        icon: None,
    }
}
