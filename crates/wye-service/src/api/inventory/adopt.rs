//! New browsers join the shown list (SHOWN-09, DISC-02): after a scan that
//! changed the apps, the browsers discovery has not offered before are
//! appended to `browsers.shown`, and the state remembers which ones it has
//! offered. A write that loses a race with an edit of the file (SET-06) is
//! retried against the fresh configuration, which is read and judged again.

use serde_json::json;
use wye_api::Error;
use wye_core::DesktopId;
use wye_core::target_menu::{Adoption, adopt_new_browsers};

use super::targets;
use crate::api::{Result, config, state};
use crate::context::ServiceContext;

/// How often one scan's adoption is tried against a configuration that keeps
/// changing on disk (SET-06) before it is left for the next scan.
const CONFLICT_ATTEMPTS: usize = 3;

/// Offer the browsers each scan finds, one scan's worth at a time, for as
/// long as the service runs. Scans only note that they stored new apps: the
/// configuration write announces changes while it holds the configuration
/// lock, and an announcement can reach a scan, so adopting inside the scan
/// would wait on itself. A failure is logged and the next scan or start
/// tries again.
pub(super) async fn run(ctx: ServiceContext) {
    // Make sure there is a first scan to look at.
    if let Err(error) = super::current(&ctx).await {
        tracing::warn!(%error, "no apps to offer in the picker");
    }
    loop {
        ctx.inventory().scanned.notified().await;
        if let Err(error) = adopt(&ctx).await {
            tracing::warn!(%error, "cannot offer new browsers in the picker");
        }
    }
}

/// Adopt the browsers of the latest scan (SHOWN-09). An `Error::Conflict` from
/// the write (SET-06) means the file changed under it: the configuration is
/// read again and the adoption recomputed, because the edit may have changed
/// `browsers.shown`, up to [`CONFLICT_ATTEMPTS`] times. Success, a read-only
/// configuration and a lossy one record the browsers as offered; any other
/// failure, or conflicts that do not stop, are logged and record nothing.
async fn adopt(ctx: &ServiceContext) -> Result<()> {
    let scan = super::current(ctx).await?;
    let state = state::load(ctx).await.unwrap_or_else(|error| {
        // Unreadable counts as never scanned: no browser is added, so nothing
        // the user hid comes back. Recording them then fails too, because a
        // writer never replaces an unreadable file; that is logged.
        tracing::warn!(%error, "the state file is unreadable; no browser is added");
        wye_desktop::State::default()
    });
    for _ in 0..CONFLICT_ATTEMPTS {
        let config = config::current(ctx).await?;
        let services = config.pipeline.services();
        let catalog = targets::catalog(
            &scan.inventory,
            &scan.environment.xdg.locale,
            &config.config,
            services,
        );
        let excluded = targets::own_apps(&scan.inventory, services);
        let Adoption { seen, shown } = adopt_new_browsers(
            &config.config,
            &catalog,
            &excluded,
            state.seen_browsers.as_deref(),
        );
        let Some(shown) = shown else {
            return record(ctx, seen).await;
        };
        let patch = json!({ "browsers": { "shown": shown } });
        match config::apply_patch(ctx, &patch, config.revision).await {
            Ok(_) => {
                tracing::info!("new browsers added to the shown list");
                return record(ctx, seen).await;
            }
            // A declarative configuration keeps its list; the browsers
            // count as offered, so they do not appear later when the file
            // becomes writable.
            Err(Error::ReadOnly(reason) | Error::NotLossless(reason)) => {
                tracing::info!(%reason, "the configuration keeps its shown list");
                return record(ctx, seen).await;
            }
            Err(Error::Conflict(reason)) => {
                tracing::info!(%reason, "the configuration changed; offering again");
            }
            Err(error) => {
                tracing::warn!(%error, "cannot add new browsers to the shown list");
                return Ok(());
            }
        }
    }
    tracing::warn!("the configuration kept changing; new browsers wait for the next scan");
    Ok(())
}

/// Remember which browsers have been offered.
async fn record(ctx: &ServiceContext, seen: Vec<DesktopId>) -> Result<()> {
    state::update(ctx, |state| {
        Ok(wye_desktop::State {
            seen_browsers: Some(seen),
            ..state.clone()
        })
    })
    .await
    .map(|_| ())
}
