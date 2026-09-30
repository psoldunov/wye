//! Apps and targets: `GetTargets`, `GetApps`, `GetServices`,
//! `GetExpansionCatalogue`, `Rescan` and the `InventoryRevision` property
//! (TGT-02 to TGT-07, APP-03, APP-05, DLG-APP, DLG-EXP, DISC-02, BRW-06).

use wye_api::Error;

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

/// The `InventoryRevision` property.
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn revision(_ctx: &ServiceContext) -> u64 {
    0
}

/// `dev.soldunov.wye1.GetTargets`: JSON [`wye_api::targets::TargetInventory`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_targets(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetTargets"))
}

/// `dev.soldunov.wye1.GetApps`: JSON [`wye_api::apps::AppList`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_apps(_ctx: &ServiceContext, _all: bool) -> Result<String> {
    Err(Error::not_implemented("GetApps"))
}

/// `dev.soldunov.wye1.GetServices`: JSON [`wye_api::services::ServiceList`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_services(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetServices"))
}

/// `dev.soldunov.wye1.GetExpansionCatalogue`: JSON
/// [`wye_api::expansion::ExpansionCatalogue`].
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn get_expansion_catalogue(_ctx: &ServiceContext) -> Result<String> {
    Err(Error::not_implemented("GetExpansionCatalogue"))
}

/// `dev.soldunov.wye1.Rescan` (TRAY-15, BRW-06).
#[allow(
    clippy::unused_async,
    reason = "the bus layer awaits every topic function; this one has nothing to await yet"
)]
pub async fn rescan(_ctx: &ServiceContext) -> Result<()> {
    Err(Error::not_implemented("Rescan"))
}
