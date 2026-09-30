//! The one request the picker shows. A new one supersedes it (PICK-27);
//! the UI's answer names the request it belongs to, so a late answer to a
//! superseded request finds nothing.

use std::sync::{Mutex, MutexGuard, PoisonError};

use wye_core::Target;

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
    /// Every target the request shows, tiles and **Open In** alike: the
    /// only ones `PickerChose` accepts.
    pub offered: Vec<Target>,
}

/// Whether the pending request offers a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Offer {
    /// The request is pending and shows the target.
    Offered,
    /// The request is pending, but does not show the target.
    NotOffered,
    /// No such request is pending.
    NotPending,
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
    pub fn open(
        &self,
        link: Option<PendingLink>,
        offered: Vec<Target>,
    ) -> (String, Option<Pending>) {
        let mut slot = self.slot();
        slot.last += 1;
        let id = slot.last.to_string();
        let superseded = slot.current.replace(Pending {
            id: id.clone(),
            link,
            offered,
        });
        (id, superseded)
    }

    /// Whether request `id` is pending and shows `target`; nothing is
    /// taken.
    pub fn offers(&self, id: &str, target: &Target) -> Offer {
        match self
            .slot()
            .current
            .as_ref()
            .filter(|pending| pending.id == id)
        {
            Some(pending) if pending.offered.contains(target) => Offer::Offered,
            Some(_) => Offer::NotOffered,
            None => Offer::NotPending,
        }
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
        let (first, none) = registry.open(None, Vec::new());
        assert!(none.is_none());
        let (second, superseded) = registry.open(None, Vec::new());
        assert_ne!(first, second);
        assert_eq!(superseded.map(|pending| pending.id), Some(first.clone()));
        assert!(registry.take(&first).is_none());
        assert_eq!(registry.take(&second).map(|p| p.id), Some(second.clone()));
        assert!(registry.take(&second).is_none(), "answered once only");
    }

    #[test]
    fn only_the_pending_request_offers_its_targets() {
        let registry = Registry::default();
        let offered = Target::App(wye_core::DesktopId::new("a.desktop").expect("id"));
        let other = Target::App(wye_core::DesktopId::new("b.desktop").expect("id"));
        let (id, _) = registry.open(None, vec![offered.clone()]);
        assert_eq!(registry.offers(&id, &offered), Offer::Offered);
        assert_eq!(registry.offers(&id, &other), Offer::NotOffered);
        assert_eq!(registry.offers("0", &offered), Offer::NotPending);
        assert!(registry.take(&id).is_some(), "checking takes nothing");
    }

    #[test]
    fn take_current_empties_the_slot() {
        let registry = Registry::default();
        registry.open(None, Vec::new());
        assert!(registry.take_current().is_some());
        assert!(registry.take_current().is_none());
    }
}
