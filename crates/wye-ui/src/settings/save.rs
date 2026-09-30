//! Saving a merge patch with `UpdateConfig` (SET-06), for every window that
//! writes the configuration: Settings, first run, the Rules page and
//! History. One place decides what a stale `base_revision` (`Conflict`)
//! means for the change.
//!
//! RFC 7386 replaces arrays, so a patch that carries one (the rule list, the
//! shown browsers, a list of key bindings) was built from the whole array the
//! window saw. Re-sending it on a newer revision is safe only when that
//! array did not change in between: re-running the change on the same input
//! gives the same patch. When the array did change, re-running an edit that
//! names entries by position (move rule 3, toggle the second browser) would
//! touch the wrong entry, so the save is refused with
//! [`Error::Conflict`] and the window reloads. A patch without arrays touches
//! only its own keys and is sent again as it is.

use std::future::Future;

use serde_json::Value;
use wye_api::Error;
use wye_api::proxy::Wye1Proxy;

/// What the service answers when a save is refused because another writer
/// changed the same list. The window shows its own sentence for `Conflict`.
pub const CHANGED_ELSEWHERE: &str = "changed elsewhere";

/// The two calls a save makes.
pub trait ConfigApi: Sync {
    fn update_config(
        &self,
        patch: &str,
        base: u64,
    ) -> impl Future<Output = Result<u64, Error>> + Send;
    fn get_config(&self) -> impl Future<Output = Result<(String, u64), Error>> + Send;
}

impl ConfigApi for Wye1Proxy<'_> {
    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        Wye1Proxy::update_config(self, patch, base).await
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        Wye1Proxy::get_config(self).await
    }
}

/// One change to save.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// The merge patch.
    pub patch: Value,
    /// The revision the window read; 0 skips the check.
    pub base_revision: u64,
    /// The configuration the patch was built from, to tell whether its
    /// arrays changed on the way (see the module docs). `Value::Null` when
    /// the caller has none: a patch with arrays is then sent again only
    /// where the fresh configuration has no such array either.
    pub base_config: Value,
}

impl Change {
    /// A patch that sets values outright, not built from the old ones (a
    /// switch, a chosen target): safe to send again on any revision.
    #[must_use]
    pub fn unchecked(patch: Value) -> Self {
        Self {
            patch,
            base_revision: 0,
            base_config: Value::Null,
        }
    }
}

/// Save `change`; returns the configuration and revision the service holds
/// afterwards.
///
/// # Errors
///
/// `ReadOnly`, `NotLossless`, `InvalidArgs`; `Conflict` when a list the
/// patch replaces changed elsewhere, or when the retry is stale again.
pub async fn save<A: ConfigApi>(api: &A, change: &Change) -> Result<(String, u64), Error> {
    let text = change.patch.to_string();
    match api.update_config(&text, change.base_revision).await {
        Ok(_) => {}
        Err(Error::Conflict(reason)) => {
            tracing::debug!(%reason, "the configuration changed under this window");
            let (fresh, revision) = api.get_config().await?;
            if !safe_to_resend(change, &fresh) {
                return Err(Error::Conflict(CHANGED_ELSEWHERE.to_owned()));
            }
            api.update_config(&text, revision).await?;
        }
        Err(error) => return Err(error),
    }
    api.get_config().await
}

/// Whether every array `change.patch` replaces is the same in `fresh` (the
/// configuration's text now) as in the configuration it was built from.
fn safe_to_resend(change: &Change, fresh: &str) -> bool {
    let arrays = array_paths(&change.patch);
    if arrays.is_empty() {
        return true;
    }
    let Ok(fresh) = serde_json::from_str::<Value>(fresh) else {
        return false;
    };
    // Missing in both counts as unchanged: a list the file never had, such
    // as the first shown browser added to an empty configuration.
    arrays
        .iter()
        .all(|path| fresh.pointer(path) == change.base_config.pointer(path))
}

/// JSON pointers to the arrays `patch` sets (arrays are replaced whole, so
/// nothing below one is a separate change).
fn array_paths(patch: &Value) -> Vec<String> {
    fn walk(value: &Value, at: &str, found: &mut Vec<String>) {
        match value {
            Value::Array(_) => found.push(at.to_owned()),
            Value::Object(map) => {
                for (key, child) in map {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    walk(child, &format!("{at}/{escaped}"), found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(patch, "", &mut found);
    found
}

#[cfg(test)]
mod tests;
