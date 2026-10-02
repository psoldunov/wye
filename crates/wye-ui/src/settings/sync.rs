//! Talking to the service for the Settings window, without Qt: reloading
//! what changed and the buttons that act on the machine (saving a patch is
//! [`super::save`]). The bridge runs these on the D-Bus thread through
//! `service::request`; tests run them against a fake [`Api`].

use std::future::Future;

use wye_api::Error;
use wye_api::proxy::Wye1Proxy;

use super::save::ConfigApi;
use super::snapshot::Snapshot;

/// The service calls the Settings window makes; saving is
/// [`super::save::save`].
pub trait Api: ConfigApi {
    fn status(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn config_revision(&self) -> impl Future<Output = Result<u64, Error>> + Send;
    fn inventory_revision(&self) -> impl Future<Output = Result<u64, Error>> + Send;
    fn get_targets(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn get_services(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn rescan(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn make_default(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn stop_being_default(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn quit(&self) -> impl Future<Output = Result<(), Error>> + Send;
}

impl Api for Wye1Proxy<'_> {
    async fn status(&self) -> Result<String, Error> {
        Ok(Wye1Proxy::status(self).await?)
    }

    async fn config_revision(&self) -> Result<u64, Error> {
        Ok(Wye1Proxy::config_revision(self).await?)
    }

    async fn inventory_revision(&self) -> Result<u64, Error> {
        Ok(Wye1Proxy::inventory_revision(self).await?)
    }

    async fn get_targets(&self) -> Result<String, Error> {
        Wye1Proxy::get_targets(self).await
    }

    async fn get_services(&self) -> Result<String, Error> {
        Wye1Proxy::get_services(self).await
    }

    async fn rescan(&self) -> Result<(), Error> {
        Wye1Proxy::rescan(self).await
    }

    async fn make_default(&self) -> Result<(), Error> {
        Wye1Proxy::make_default(self).await
    }

    async fn stop_being_default(&self) -> Result<(), Error> {
        Wye1Proxy::stop_being_default(self).await
    }

    async fn quit(&self) -> Result<(), Error> {
        Wye1Proxy::quit(self).await
    }
}

/// The revisions the window already shows. Zero is never reported, so it
/// asks for everything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Known {
    pub config: u64,
    pub inventory: u64,
    /// The configuration revision the targets and services were read at.
    /// They name the configuration's custom apps and missing targets
    /// (TGT-06, APP-10), which `inventory` does not count, so a config move
    /// reads them again. Zero is unknown.
    pub inventory_config: u64,
}

/// Targets and services with the revisions they belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub targets: String,
    pub services: String,
    pub revision: u64,
    /// The configuration revision read just before the targets.
    pub config_revision: u64,
}

/// What changed on the service since [`Known`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    pub status: String,
    pub config: Option<(String, u64)>,
    pub inventory: Option<Inventory>,
}

impl Delta {
    /// The revisions after this delta is applied to a window that knew
    /// `known`.
    #[must_use]
    pub fn known(&self, known: Known) -> Known {
        Known {
            config: self
                .config
                .as_ref()
                .map_or(known.config, |(_, revision)| *revision),
            inventory: self
                .inventory
                .as_ref()
                .map_or(known.inventory, |i| i.revision),
            inventory_config: self
                .inventory
                .as_ref()
                .map_or(known.inventory_config, |i| i.config_revision),
        }
    }

    /// `snapshot` with this delta applied.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when the service sent something that is not the
    /// expected JSON.
    pub fn apply(&self, snapshot: &Snapshot) -> Result<Snapshot, Error> {
        let mut next = snapshot.with_status(&self.status)?;
        if let Some((config, revision)) = &self.config {
            next = next.with_config(config, *revision)?;
        }
        if let Some(inventory) = &self.inventory {
            next = next.with_inventory(&inventory.targets, &inventory.services)?;
        }
        Ok(next)
    }
}

/// Read what changed: the status always (default-browser state and the
/// configuration's health change without a revision), the configuration and
/// the inventory only when its revision or the configuration's moved (the
/// targets list the configuration's custom apps, TGT-06).
///
/// # Errors
///
/// Whatever the service returns.
pub async fn poll<A: Api>(api: &A, known: Known) -> Result<Delta, Error> {
    let status = api.status().await?;
    let config_revision = api.config_revision().await?;
    let config = if config_revision == known.config {
        None
    } else {
        Some(api.get_config().await?)
    };
    let inventory_revision = api.inventory_revision().await?;
    let inventory =
        if inventory_revision == known.inventory && config_revision == known.inventory_config {
            None
        } else {
            Some(Inventory {
                targets: api.get_targets().await?,
                services: api.get_services().await?,
                revision: inventory_revision,
                config_revision,
            })
        };
    Ok(Delta {
        status,
        config,
        inventory,
    })
}

/// A button that acts on the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// GEN-05: make Wye the default browser.
    MakeDefault,
    /// GEN-05: restore the previous default.
    StopBeingDefault,
    /// BRW-06: look for apps and profiles again.
    Rescan,
    /// KEY-50 `Ctrl+Q`: stop the Wye service (TRAY-17).
    Quit,
}

impl Action {
    /// The name QML passes.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "make-default" => Some(Self::MakeDefault),
            "stop-being-default" => Some(Self::StopBeingDefault),
            "rescan" => Some(Self::Rescan),
            "quit" => Some(Self::Quit),
            _ => None,
        }
    }
}

/// Do `action`, then read what it changed. Quitting the service changes
/// nothing to read: `None`.
///
/// # Errors
///
/// Whatever the service returns: `ReadOnly` when `mimeapps.list` is managed
/// elsewhere, `NotFound` when there is nothing to restore.
pub async fn run<A: Api>(api: &A, action: Action, known: Known) -> Result<Option<Delta>, Error> {
    match action {
        Action::MakeDefault => api.make_default().await?,
        Action::StopBeingDefault => api.stop_being_default().await?,
        Action::Rescan => api.rescan().await?,
        Action::Quit => {
            api.quit().await?;
            return Ok(None);
        }
    }
    poll(api, known).await.map(Some)
}

#[cfg(test)]
mod tests;
