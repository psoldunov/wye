//! The one request the picker shows. A new one supersedes it (PICK-27);
//! the UI's answer names the request it belongs to, so a late answer to a
//! superseded request finds nothing.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::api::link::{Activation, PickerNeeded};

/// A link waiting for the picker's choice.
#[derive(Debug, Clone)]
pub(crate) struct PendingLink {
    pub needed: PickerNeeded,
    /// The activation data the link arrived with, used when the picker
    /// supplies no token of its own (LAUNCH-03).
    pub activation: Activation,
}

/// What the picker shows now.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub id: String,
    /// `None` for Preview Picker (PKS-06): choosing opens nothing.
    pub link: Option<PendingLink>,
}

#[derive(Debug, Default)]
struct Slot {
    /// The last request ID handed out.
    last: u64,
    current: Option<Pending>,
}

/// Request IDs and the current request.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    slot: Mutex<Slot>,
}

impl Registry {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Make `link` the current request under a new ID, superseding the
    /// previous one (PICK-27), which is returned.
    pub fn open(&self, link: Option<PendingLink>) -> (String, Option<Pending>) {
        let mut slot = self.slot();
        slot.last += 1;
        let id = slot.last.to_string();
        let superseded = slot.current.replace(Pending {
            id: id.clone(),
            link,
        });
        (id, superseded)
    }

    /// Remove and return the request `id`, when it is still the current one.
    pub fn take(&self, id: &str) -> Option<Pending> {
        let mut slot = self.slot();
        if slot
            .current
            .as_ref()
            .is_some_and(|pending| pending.id == id)
        {
            slot.current.take()
        } else {
            None
        }
    }

    /// Remove and return the current request, whatever its ID.
    pub fn take_current(&self) -> Option<Pending> {
        self.slot().current.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_request_supersedes_the_pending_one() {
        // PICK-27: the old request's answer then finds nothing.
        let registry = Registry::default();
        let (first, none) = registry.open(None);
        assert!(none.is_none());
        let (second, superseded) = registry.open(None);
        assert_ne!(first, second);
        assert_eq!(superseded.map(|pending| pending.id), Some(first.clone()));
        assert!(registry.take(&first).is_none());
        assert_eq!(registry.take(&second).map(|p| p.id), Some(second.clone()));
        assert!(registry.take(&second).is_none(), "answered once only");
    }

    #[test]
    fn take_current_empties_the_slot() {
        let registry = Registry::default();
        registry.open(None);
        assert!(registry.take_current().is_some());
        assert!(registry.take_current().is_none());
    }
}
