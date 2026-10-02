//! Windows and lifetime: `org.freedesktop.Application.Activate` and
//! `ActivateAction`, `ShowWindow`, `Quit` (TRAY-05, TRAY-16, TRAY-17,
//! SET-04).
//!
//! Windows belong to a UI host: on GNOME use activatable GTK when installed;
//! otherwise use the existing activatable Qt host.

use wye_api::Error;
use wye_api::actions::{ApplicationAction, Window};
use zbus::zvariant::OwnedValue;

use super::{Caller, Dict, Result};
use crate::context::ServiceContext;

/// `org.freedesktop.Application.Activate`: Wye started without a link
/// (TRAY-05).
///
/// The first-run window while onboarding is not done (ONB-06); otherwise
/// Settings while no tray icon is on screen, so the user can always get back
/// in; otherwise nothing, since the tray icon is the way in. A UI host that
/// cannot be reached is logged, not returned: a launcher has nobody to show
/// the error to.
pub async fn activate(ctx: &ServiceContext, _caller: &Caller, _platform_data: &Dict) -> Result<()> {
    let onboarded = super::state::load(ctx)
        .await
        .inspect_err(|error| tracing::warn!(%error, "cannot read the onboarding state"))
        .is_ok_and(|state| state.onboarding_done);
    let window = if !onboarded {
        Window::FirstRun
    } else if !super::tray::icon_on_screen(ctx).await {
        Window::Settings
    } else {
        return Ok(());
    };
    if let Err(error) = show(ctx, window, "").await {
        tracing::warn!(%error, window = window.as_str(), "cannot open the window");
    }
    Ok(())
}

/// `org.freedesktop.Application.ActivateAction`: a desktop action
/// ([`wye_api::actions::ApplicationAction`]).
pub async fn activate_action(
    ctx: &ServiceContext,
    caller: &Caller,
    action: &str,
    _parameter: &[OwnedValue],
    _platform_data: &Dict,
) -> Result<()> {
    let action: ApplicationAction = action
        .parse()
        .map_err(|error| Error::invalid_args(format!("{error}")))?;
    match action {
        ApplicationAction::Settings => show(ctx, Window::Settings, "").await,
        ApplicationAction::Setup => show(ctx, Window::FirstRun, "").await,
        ApplicationAction::History => show(ctx, Window::History, "").await,
        ApplicationAction::TestRules => show(ctx, Window::TestRules, "").await,
        ApplicationAction::About => show(ctx, Window::About, "").await,
        ApplicationAction::Clipboard => super::clipboard::open_clipboard(ctx, caller, false).await,
        ApplicationAction::ClipboardAlternative => {
            super::clipboard::open_clipboard(ctx, caller, true).await
        }
        ApplicationAction::Menu => super::shortcuts::toggle_menu(ctx).await,
        ApplicationAction::Quit => quit(ctx).await,
    }
}

/// `dev.soldunov.wye1.ShowWindow` ([`wye_api::actions::Window`]).
pub async fn show_window(ctx: &ServiceContext, window: &str, argument: &str) -> Result<()> {
    let window: Window = window
        .parse()
        .map_err(|error| Error::invalid_args(format!("{error}")))?;
    show(ctx, window, argument).await
}

/// Forward to the UI host; `Unavailable` when it cannot be reached.
async fn show(ctx: &ServiceContext, window: Window, argument: &str) -> Result<()> {
    super::picker::host::show_window(ctx, window, argument)
        .await
        .map_err(|error| {
            Error::Unavailable(format!(
                "the Wye window host did not open {window}: {error}"
            ))
        })
}

/// `dev.soldunov.wye1.Quit` (TRAY-17): the service stops; the next link
/// starts it again through D-Bus activation.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await"
)]
pub async fn quit(ctx: &ServiceContext) -> Result<()> {
    ctx.request_shutdown();
    Ok(())
}
