//! Apps and targets: `GetTargets`, `GetApps`, `GetServices`,
//! `GetExpansionCatalogue`, `Rescan` and the `InventoryRevision` property
//! (TGT-02 to TGT-07, APP-03, APP-05, APP-10, DLG-APP, DLG-EXP, DISC-02,
//! BRW-06, TRAY-15, SHOWN-09).
//!
//! The installed apps and browser profiles are scanned once and kept; the
//! watchers ([`crate::watch`]) and `Rescan` scan again, and the revision
//! bumps only when something changed.

mod adopt;
mod lists;
pub(crate) mod targets;

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use tokio::task::JoinHandle;
use wye_api::json;
use wye_core::{DesktopId, ServiceCatalogue};
use wye_desktop::{Inventory, WYE_DESKTOP_ID};

use super::Result;
use crate::api::link::Environment;
use crate::bus::Property;
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// How many recent source apps the app chooser offers (DLG-APP-02).
const RECENT_SOURCES: usize = 10;

/// One scan of the installed apps.
#[derive(Debug)]
pub(crate) struct Scan {
    pub environment: Arc<Environment>,
    pub inventory: Arc<Inventory>,
    pub revision: u64,
}

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    current: RwLock<Option<Arc<Scan>>>,
    /// One scan at a time.
    scanning: tokio::sync::Mutex<()>,
    /// A scan stored new apps, so the adoption task has browsers to look at
    /// (SHOWN-09).
    scanned: tokio::sync::Notify,
    /// Desktop IDs of apps that sent links, newest first (DLG-APP-02); kept
    /// in memory only.
    recent_sources: Mutex<Vec<String>>,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }

    fn get(&self) -> Option<Arc<Scan>> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, scan: Arc<Scan>) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Some(scan);
    }

    /// Desktop IDs of the apps that recently sent links, newest first.
    pub(crate) fn recent_sources(&self) -> Vec<String> {
        self.recent_sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The task that offers new browsers in the picker (SHOWN-09); see
/// [`adopt::run`].
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![tokio::spawn(adopt::run(ctx.clone()))]
}

/// The apps, scanned on first use and again whenever the environment
/// changes.
///
/// # Errors
///
/// `Failed` when there is no home directory.
pub(crate) async fn current(ctx: &ServiceContext) -> Result<Arc<Scan>> {
    let environment = ctx.environment()?;
    if let Some(scan) = ctx.inventory().get()
        && Arc::ptr_eq(&scan.environment, &environment)
    {
        return Ok(scan);
    }
    scan(ctx).await
}

/// Scan now (DISC-02, BRW-06). Clients hear about it only when apps or
/// profiles changed.
///
/// # Errors
///
/// `Failed` when there is no home directory.
pub(crate) async fn scan(ctx: &ServiceContext) -> Result<Arc<Scan>> {
    let scanning = ctx.inventory().scanning.lock().await;
    let environment = ctx.environment()?;
    let before = ctx.inventory().get();
    let xdg = environment.xdg.clone();
    let inventory = blocking(move || {
        DesktopId::new(WYE_DESKTOP_ID).map_or_else(
            |_| Inventory::from_apps(Vec::new(), Vec::new()),
            |wye| Inventory::scan(&xdg, &wye),
        )
    })
    .await?;
    for warning in inventory.warnings() {
        tracing::info!(%warning, "app discovery");
    }
    if let Some(before) = &before
        && Arc::ptr_eq(&before.environment, &environment)
        && same_apps(&before.inventory, &inventory)
    {
        return Ok(before.clone());
    }
    let scan = Arc::new(Scan {
        revision: before.as_ref().map_or(1, |before| before.revision + 1),
        environment,
        inventory: Arc::new(inventory),
    });
    ctx.inventory().set(scan.clone());
    drop(scanning);
    ctx.inventory().scanned.notify_one();
    if before.is_some() {
        super::config::effects::changed(ctx, Property::InventoryRevision);
        super::config::effects::changed(ctx, Property::Tray);
        super::config::effects::changed(ctx, Property::Status);
    }
    Ok(scan)
}

fn same_apps(a: &Inventory, b: &Inventory) -> bool {
    a.apps().eq(b.apps()) && a.warnings() == b.warnings()
}

/// Note that `source` (a desktop ID) sent a link, for the app chooser's
/// recent list (DLG-APP-02).
pub(crate) fn remember_source(ctx: &ServiceContext, source: &str) {
    if source.is_empty() {
        return;
    }
    let mut recent = ctx
        .inventory()
        .recent_sources
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let next: Vec<String> = std::iter::once(source.to_owned())
        .chain(recent.iter().filter(|id| *id != source).cloned())
        .take(RECENT_SOURCES)
        .collect();
    *recent = next;
}

/// The `InventoryRevision` property; 0 before the first scan could run.
pub async fn revision(ctx: &ServiceContext) -> u64 {
    current(ctx).await.map_or(0, |scan| scan.revision)
}

/// `dev.soldunov.wye1.GetTargets`: JSON [`wye_api::targets::TargetInventory`].
pub async fn get_targets(ctx: &ServiceContext) -> Result<String> {
    with_config(ctx, |inventory, locale, config, services| {
        json::encode(&targets::inventory(inventory, locale, config, services))
    })
    .await
}

/// Run `build` over the apps, the locale, the configuration and the web
/// app catalogue.
async fn with_config(
    ctx: &ServiceContext,
    build: impl FnOnce(
        &Inventory,
        &wye_desktop::Locale,
        &wye_core::Config,
        &ServiceCatalogue,
    ) -> Result<String>,
) -> Result<String> {
    let scan = current(ctx).await?;
    let config = super::config::current(ctx).await?;
    build(
        &scan.inventory,
        &scan.environment.xdg.locale,
        &config.config,
        config.pipeline.services(),
    )
}

/// `dev.soldunov.wye1.GetApps`: JSON [`wye_api::apps::AppList`].
pub async fn get_apps(ctx: &ServiceContext, all: bool) -> Result<String> {
    let scan = current(ctx).await?;
    let locale = &scan.environment.xdg.locale;
    json::encode(&lists::apps(
        &scan.inventory,
        locale,
        all,
        ctx.inventory().recent_sources(),
    ))
}

/// `dev.soldunov.wye1.GetServices`: JSON [`wye_api::services::ServiceList`].
pub async fn get_services(ctx: &ServiceContext) -> Result<String> {
    with_config(ctx, |inventory, locale, config, services| {
        json::encode(&lists::services(inventory, locale, config, services))
    })
    .await
}

/// `dev.soldunov.wye1.GetExpansionCatalogue`: JSON
/// [`wye_api::expansion::ExpansionCatalogue`].
pub async fn get_expansion_catalogue(ctx: &ServiceContext) -> Result<String> {
    let config = super::config::current(ctx).await?;
    json::encode(&lists::expansion(
        &wye_core::expand::ExpansionCatalogue::shipped(),
        &config.config.advanced.expansion,
    ))
}

/// `dev.soldunov.wye1.Rescan` (TRAY-15, BRW-06).
pub async fn rescan(ctx: &ServiceContext) -> Result<()> {
    scan(ctx).await.map(|_| ())
}
