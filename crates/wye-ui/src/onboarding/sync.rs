//! Talking to the service for the first-run window, without Qt: reading
//! what the steps show and carrying out what the user chooses (ONB-02 to
//! ONB-06). The bridge runs these on the D-Bus thread through
//! `service::request`; tests run them against a fake [`Api`].
//!
//! Every choice is stored as it is made, so closing the window early loses
//! nothing (ONB-06).

use std::future::Future;

use serde_json::{Value, json};
use wye_api::Error;
use wye_api::proxy::Wye1Proxy;

use crate::settings::snapshot::Snapshot;

/// The UI state patch that ends the first run (ONB-06).
fn done_patch() -> Value {
    json!({"onboardingDone": true})
}

/// The service calls the first-run window makes.
pub trait Api: Sync {
    fn status(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn get_config(&self) -> impl Future<Output = Result<(String, u64), Error>> + Send;
    fn update_config(
        &self,
        patch: &str,
        base: u64,
    ) -> impl Future<Output = Result<u64, Error>> + Send;
    fn get_targets(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn get_services(&self) -> impl Future<Output = Result<String, Error>> + Send;
    fn make_default(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn keep_current_default(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn update_ui_state(&self, patch: &str) -> impl Future<Output = Result<(), Error>> + Send;
}

impl Api for Wye1Proxy<'_> {
    async fn status(&self) -> Result<String, Error> {
        Ok(Wye1Proxy::status(self).await?)
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        Wye1Proxy::get_config(self).await
    }

    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        Wye1Proxy::update_config(self, patch, base).await
    }

    async fn get_targets(&self) -> Result<String, Error> {
        Wye1Proxy::get_targets(self).await
    }

    async fn get_services(&self) -> Result<String, Error> {
        Wye1Proxy::get_services(self).await
    }

    async fn make_default(&self) -> Result<(), Error> {
        Wye1Proxy::make_default(self).await
    }

    async fn keep_current_default(&self) -> Result<(), Error> {
        Wye1Proxy::keep_current_default(self).await
    }

    async fn update_ui_state(&self, patch: &str) -> Result<(), Error> {
        Wye1Proxy::update_ui_state(self, patch).await
    }
}

/// Read everything the steps show.
///
/// # Errors
///
/// The service's error, or a reply that is not the expected JSON.
pub async fn load<A: Api>(api: &A) -> Result<Snapshot, Error> {
    let (config, revision) = api.get_config().await?;
    let status = api.status().await?;
    let targets = api.get_targets().await?;
    let services = api.get_services().await?;
    Snapshot::default()
        .with_config(&config, revision)?
        .with_status(&status)?
        .with_inventory(&targets, &services)
}

/// Apply `patch` to the configuration on top of the revision `snapshot`
/// knows. A stale revision (`Conflict`) reads the fresh one and applies the
/// patch again once: merge patches touch only their own keys, so
/// re-applying is safe.
async fn save<A: Api>(api: &A, snapshot: &Snapshot, patch: &Value) -> Result<Snapshot, Error> {
    let text = patch.to_string();
    match api.update_config(&text, snapshot.revision).await {
        Ok(_) => {}
        Err(Error::Conflict(reason)) => {
            tracing::debug!(%reason, "configuration changed under us; applying the patch again");
            let (_, fresh) = api.get_config().await?;
            api.update_config(&text, fresh).await?;
        }
        Err(error) => return Err(error),
    }
    let (config, revision) = api.get_config().await?;
    snapshot.with_config(&config, revision)
}

/// What a step's button does on the service.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// **Make Default** (ONB-02, DEF-02).
    MakeDefault,
    /// **Skip** on the default-browser step (ONB-02): keep the current
    /// default and stop asking (ONB-11).
    KeepCurrentDefault,
    /// A choice on the browsers or integration step (ONB-03, ONB-04): a
    /// merge patch of the configuration.
    Patch(Value),
    /// The walk-through is over, or the window was closed early (ONB-06).
    Finish,
}

/// Do `action`, then return `snapshot` as the service holds it now.
///
/// # Errors
///
/// `ReadOnly` when `mimeapps.list` or the configuration is managed
/// elsewhere, `NotLossless`, `Failed` when Wye's desktop entry is not
/// installed, or a second `Conflict`.
pub async fn run<A: Api>(api: &A, action: Action, snapshot: &Snapshot) -> Result<Snapshot, Error> {
    match action {
        Action::MakeDefault => {
            api.make_default().await?;
            snapshot.with_status(&api.status().await?)
        }
        Action::KeepCurrentDefault => {
            api.keep_current_default().await?;
            snapshot.with_status(&api.status().await?)
        }
        Action::Patch(patch) => save(api, snapshot, &patch).await,
        Action::Finish => {
            api.update_ui_state(&done_patch().to_string()).await?;
            Ok(snapshot.with_ui_state_patch(&done_patch()))
        }
    }
}

/// `snapshot` as it will look once `action` succeeds, so the window shows a
/// choice at once. Only what the window itself changes: the configuration
/// patch, the finished flag, and the default-browser status (which the
/// bridge previews only when there is no service to ask).
#[must_use]
pub fn preview(snapshot: &Snapshot, action: &Action) -> Snapshot {
    match action {
        Action::Patch(patch) => snapshot.with_patch(patch, snapshot.revision),
        Action::Finish => snapshot.with_ui_state_patch(&done_patch()),
        Action::MakeDefault => with_default_browser(snapshot, |registration| {
            registration.is_default = true;
        }),
        Action::KeepCurrentDefault => with_default_browser(snapshot, |registration| {
            registration.kept_current = true;
        }),
    }
}

fn with_default_browser(
    snapshot: &Snapshot,
    change: impl FnOnce(&mut wye_api::status::DefaultBrowserStatus),
) -> Snapshot {
    let mut registration = snapshot.status.default_browser.clone();
    change(&mut registration);
    Snapshot {
        status: wye_api::status::Status {
            default_browser: registration,
            ..snapshot.status.clone()
        },
        ..snapshot.clone()
    }
}

#[cfg(test)]
mod tests;
