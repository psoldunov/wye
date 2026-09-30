//! External tray hosts that registered with `RegisterTray` (decision 8):
//! while one lives, the service shows no `StatusNotifierItem`. A host is
//! forgotten when its bus connection goes away. Wye ships no such host; the
//! call stays for compatibility with older applets and third-party hosts.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use futures_lite::StreamExt as _;
use tokio::sync::watch;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

/// One bus connection that called `RegisterTray`.
#[derive(Debug, Clone, Copy)]
struct Host {
    /// `RegisterTray` calls not yet matched by `UnregisterTray`. Several
    /// instances of one host (applets in one panel process) share one
    /// connection, so one instance going away must not bring the item back
    /// while another still shows the tray (decision 8).
    registrations: u32,
}

/// The registered hosts, by unique bus name. A connection stays listed
/// with no registrations until it leaves the bus, so it is watched once.
#[derive(Debug)]
pub(crate) struct Hosts {
    registered: Mutex<BTreeMap<String, Host>>,
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
    /// Count one more registration from `name`. True when `name` was not
    /// known yet, so its connection needs watching.
    pub(crate) fn insert(&self, name: &str) -> bool {
        let (new, first) = {
            let mut registered = self.lock();
            let new = !registered.contains_key(name);
            let host = registered
                .entry(name.to_owned())
                .or_insert(Host { registrations: 0 });
            host.registrations = host.registrations.saturating_add(1);
            (new, host.registrations == 1)
        };
        if first {
            self.bump();
        }
        new
    }

    /// One registration from `name` fewer (`UnregisterTray`). True when it
    /// was the last one, so `name` no longer hosts the tray.
    pub(crate) fn remove(&self, name: &str) -> bool {
        let last = {
            let mut registered = self.lock();
            match registered.get_mut(name) {
                Some(host) if host.registrations > 0 => {
                    host.registrations -= 1;
                    host.registrations == 0
                }
                _ => false,
            }
        };
        if last {
            self.bump();
        }
        last
    }

    /// `name` left the bus: forget it and all its registrations.
    pub(crate) fn forget(&self, name: &str) {
        let hosted = self
            .lock()
            .remove(name)
            .is_some_and(|host| host.registrations > 0);
        if hosted {
            self.bump();
        }
    }

    /// Whether any tray host is registered.
    pub(crate) fn any(&self) -> bool {
        self.lock().values().any(|host| host.registrations > 0)
    }

    /// Wakes on every change.
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn bump(&self) {
        self.changes
            .send_modify(|count| *count = count.wrapping_add(1));
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Host>> {
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
        assert!(hosts.insert(":1.5"), "new: watch it");
        assert!(hosts.any());
        assert!(changes.has_changed().expect("sender alive"));
        changes.mark_unchanged();
        assert!(hosts.remove(":1.5"), "the last registration");
        assert!(!hosts.any());
        assert!(changes.has_changed().expect("sender alive"));
        assert!(!hosts.remove(":1.5"), "nothing left to remove");
    }

    #[test]
    fn decision_8_every_registration_of_one_connection_counts() {
        // Two host instances on one connection.
        let hosts = Hosts::default();
        assert!(hosts.insert(":1.5"));
        assert!(!hosts.insert(":1.5"), "watched already");
        assert!(!hosts.remove(":1.5"), "one instance is still there");
        assert!(hosts.any());
        assert!(hosts.remove(":1.5"));
        assert!(!hosts.any());
        // Registering again later needs no second watch.
        assert!(!hosts.insert(":1.5"));
        assert!(hosts.any());
        hosts.forget(":1.5");
        assert!(!hosts.any(), "leaving the bus ends every registration");
        assert!(hosts.insert(":1.5"), "a new connection");
    }
}
