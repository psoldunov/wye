//! Talking to the service for the history window, without Qt: reloading
//! when `HistoryRevision` moves (DLG-HIS-01) and the actions of DLG-HIS-01,
//! DLG-HIS-03 and DLG-HIS-04. The bridge runs these on the D-Bus thread
//! through `service::request`; tests run them against a fake [`Api`].

use std::future::Future;

use wye_api::actions::{Reopen, Window};
use wye_api::history::History;
use wye_api::proxy::Wye1Proxy;
use wye_api::targets::TargetInventory;
use wye_api::{Error, json};

/// The merge patch that switches history on (ADV-09).
const TURN_ON_PATCH: &str = r#"{"advanced":{"history":true}}"#;

/// The service calls the history window makes.
pub trait Api: Sync {
    fn history_revision(&self) -> impl Future<Output = Result<u64, Error>> + Send;
    fn inventory_revision(&self) -> impl Future<Output = Result<u64, Error>> + Send;
    fn get_history(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn get_targets(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn clear_history(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn delete_history_entry(&self, id: u64) -> impl Future<Output = Result<(), Error>> + Send;
    fn reopen_history_entry(
        &self,
        id: u64,
        how: &str,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    fn get_config(&self) -> impl Future<Output = Result<(String, u64), Error>> + Send;
    fn update_config(
        &self,
        patch: &str,
        base: u64,
    ) -> impl Future<Output = Result<u64, Error>> + Send;
    fn show_window(
        &self,
        window: &str,
        argument: &str,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

impl Api for Wye1Proxy<'_> {
    async fn history_revision(&self) -> Result<u64, Error> {
        Ok(Wye1Proxy::history_revision(self).await?)
    }

    async fn inventory_revision(&self) -> Result<u64, Error> {
        Ok(Wye1Proxy::inventory_revision(self).await?)
    }

    async fn get_history(&self) -> Result<String, Error> {
        Wye1Proxy::get_history(self).await
    }

    async fn get_targets(&self) -> Result<String, Error> {
        Wye1Proxy::get_targets(self).await
    }

    async fn clear_history(&self) -> Result<(), Error> {
        Wye1Proxy::clear_history(self).await
    }

    async fn delete_history_entry(&self, id: u64) -> Result<(), Error> {
        Wye1Proxy::delete_history_entry(self, id).await
    }

    async fn reopen_history_entry(&self, id: u64, how: &str) -> Result<(), Error> {
        Wye1Proxy::reopen_history_entry(self, id, how).await
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        Wye1Proxy::get_config(self).await
    }

    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        Wye1Proxy::update_config(self, patch, base).await
    }

    async fn show_window(&self, window: &str, argument: &str) -> Result<(), Error> {
        Wye1Proxy::show_window(self, window, argument).await
    }
}

/// The revisions the window already shows. Zero is never reported, so it
/// asks for everything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Known {
    pub history: u64,
    pub inventory: u64,
}

/// What the window shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub history: History,
    pub targets: TargetInventory,
    pub known: Known,
}

/// What changed on the service since [`Known`].
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub history: Option<(History, u64)>,
    pub targets: Option<(TargetInventory, u64)>,
}

impl Update {
    /// `snapshot` with this update applied.
    #[must_use]
    pub fn apply(self, snapshot: &Snapshot) -> Snapshot {
        let mut next = snapshot.clone();
        if let Some((history, revision)) = self.history {
            next.history = history;
            next.known.history = revision;
        }
        if let Some((targets, revision)) = self.targets {
            next.targets = targets;
            next.known.inventory = revision;
        }
        next
    }
}

/// Read what changed since `known`: nothing when both revisions are still
/// the same (DLG-HIS-01, live refresh).
///
/// # Errors
///
/// The service's error, or a reply that is not the expected JSON.
pub async fn poll<A: Api>(api: &A, known: Known) -> Result<Option<Update>, Error> {
    let history_revision = api.history_revision().await?;
    let history = if history_revision == known.history {
        None
    } else {
        let text = api.get_history().await?;
        Some((json::decode::<History>("history", &text)?, history_revision))
    };
    let inventory_revision = api.inventory_revision().await?;
    let targets = if inventory_revision == known.inventory {
        None
    } else {
        let text = api.get_targets().await?;
        Some((
            json::decode::<TargetInventory>("targets", &text)?,
            inventory_revision,
        ))
    };
    Ok((history.is_some() || targets.is_some()).then_some(Update { history, targets }))
}

/// What the buttons and menu items do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// **Clear History** (DLG-HIS-01).
    Clear,
    /// **Delete Entry** (DLG-HIS-03).
    Delete(u64),
    /// **Open in Picker** and **Open in \<target\> Again** (DLG-HIS-03).
    Reopen(u64, Reopen),
    /// **Turn On** in the "History Is Off" state (DLG-HIS-04).
    TurnOn,
    /// **Create Rule…** (DLG-HIS-03): the rule editor, pre-filled.
    CreateRule { prefill: String },
}

/// Carry out `action`, then read what it changed. `known` is what the window
/// shows now.
///
/// # Errors
///
/// The service's error from the action or the reload.
pub async fn run<A: Api>(api: &A, action: Action, known: Known) -> Result<Option<Update>, Error> {
    let known = match action {
        Action::Clear => {
            api.clear_history().await?;
            known
        }
        Action::Delete(id) => {
            api.delete_history_entry(id).await?;
            known
        }
        Action::Reopen(id, how) => {
            api.reopen_history_entry(id, how.as_str()).await?;
            known
        }
        Action::TurnOn => {
            let (_, revision) = api.get_config().await?;
            api.update_config(TURN_ON_PATCH, revision).await?;
            // The switch may not bump `HistoryRevision`: read everything.
            Known::default()
        }
        Action::CreateRule { prefill } => {
            api.show_window(Window::RuleEditor.as_str(), &prefill)
                .await?;
            return Ok(None);
        }
    };
    poll(api, known).await
}

#[cfg(test)]
mod tests;
