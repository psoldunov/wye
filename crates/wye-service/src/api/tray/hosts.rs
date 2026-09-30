//! Tray hosts that registered with `RegisterTray` (decision 8): while one
//! lives, the service shows no `StatusNotifierItem`. A host is forgotten
//! when its bus connection goes away.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use futures_lite::StreamExt as _;
use tokio::sync::watch;
use wye_api::actions::TrayHost;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

/// The registered hosts, by unique bus name.
#[derive(Debug)]
pub(crate) struct Hosts {
    registered: Mutex<BTreeMap<String, TrayHost>>,
    /// Bumped on every change, for the tray task.
    changes: watch::Sender<u64>,
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            registered: Mutex::default(),
            changes: watch::Sender::new(0),
        }
    }
}

impl Hosts {
    /// Remember `name` as a host of `kind`; false when it already was one.
    pub(crate) fn insert(&self, name: &str, kind: TrayHost) -> bool {
        let added = self.lock().insert(name.to_owned(), kind).is_none();
        if added {
            self.bump();
        }
        added
    }

    /// Forget `name`; false when it was not a host.
    pub(crate) fn remove(&self, name: &str) -> bool {
        let removed = self.lock().remove(name).is_some();
        if removed {
            self.bump();
        }
        removed
    }

    /// Whether any tray host is registered.
    pub(crate) fn any(&self) -> bool {
        !self.lock().is_empty()
    }

    /// Wakes on every change.
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn bump(&self) {
        self.changes
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, TrayHost>> {
        self.registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Resolves when `name` leaves `connection`'s bus. Errors count as gone:
/// a host the service cannot watch must not hide the tray for good.
pub(crate) async fn until_gone(connection: &zbus::Connection, name: &str) {
    if let Err(error) = watch_owner(connection, name).await {
        tracing::warn!(%error, name, "cannot watch the tray host; forgetting it");
    }
}

async fn watch_owner(connection: &zbus::Connection, name: &str) -> zbus::Result<()> {
    let bus = DBusProxy::new(connection).await?;
    let mut changes = bus
        .receive_name_owner_changed_with_args(&[(0, name)])
        .await?;
    // It may have left between its call and the subscription.
    if !bus.name_has_owner(BusName::try_from(name)?).await? {
        return Ok(());
    }
    while let Some(change) = changes.next().await {
        if change.args()?.new_owner().is_none() {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_come_and_go_and_announce_it() {
        let hosts = Hosts::default();
        let mut changes = hosts.subscribe();
        assert!(!hosts.any());
        assert!(hosts.insert(":1.5", TrayHost::PlasmaApplet));
        assert!(!hosts.insert(":1.5", TrayHost::PlasmaApplet), "once");
        assert!(hosts.any());
        assert!(changes.has_changed().expect("sender alive"));
        changes.mark_unchanged();
        hosts.remove(":1.5");
        assert!(!hosts.any());
        assert!(changes.has_changed().expect("sender alive"));
    }
}
