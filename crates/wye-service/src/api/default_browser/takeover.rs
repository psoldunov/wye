//! DEF-03, ONB-11: another app took over as the default browser. One
//! notification per takeover with "Make Wye Default" and "Keep <App>",
//! none while the user chose to keep that app.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::task::JoinHandle;
use wye_core::DesktopId;

use super::detect::Registration;
use crate::context::ServiceContext;
use crate::platform::Notification;

/// Action key of "Make Wye Default".
pub(crate) const MAKE_DEFAULT_ACTION: &str = "wye-make-default";
/// Action key of "Keep <App>".
pub(crate) const KEEP_ACTION: &str = "wye-keep-default";

/// Wye stopped being the default; `app` has the links now, when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TookOver {
    pub app: Option<DesktopId>,
}

/// What takeover detection remembers.
#[derive(Debug, Default)]
pub(crate) struct Watch {
    /// The registration last seen; `None` before the first look.
    last: Mutex<Option<Registration>>,
    /// The takeover notification on screen and the app it names.
    shown: Mutex<Option<(u32, DesktopId)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Watch {
    /// Remember `now`; returns the app that took over when Wye just stopped
    /// being the default.
    pub fn saw(&self, now: Registration) -> Option<TookOver> {
        let before = lock(&self.last).replace(now.clone());
        let was_default = before.is_some_and(|before| before.is_default);
        (was_default && !now.is_default).then_some(TookOver { app: now.current })
    }

    pub fn shown(&self, id: u32, app: DesktopId) {
        *lock(&self.shown) = Some((id, app));
    }

    /// The app named by notification `id`, forgetting it.
    pub fn answered(&self, id: u32) -> Option<DesktopId> {
        let mut shown = lock(&self.shown);
        match shown.as_ref() {
            Some((shown_id, _)) if *shown_id == id => shown.take().map(|(_, app)| app),
            _ => None,
        }
    }

    /// The notification on screen, still remembered: a newer one replaces it
    /// only once that is shown (DEF-03), so a failed `notify` leaves its
    /// buttons working.
    pub fn on_screen(&self) -> Option<u32> {
        lock(&self.shown).as_ref().map(|(id, _)| *id)
    }

    /// The notification on screen, forgetting it: its takeover was fixed or
    /// kept another way, so its buttons answer nothing any more.
    pub fn withdraw(&self) -> Option<u32> {
        lock(&self.shown).take().map(|(id, _)| id)
    }
}

/// DEF-03, ONB-11: withdraw the takeover notification, if one is shown.
pub(crate) async fn withdraw(ctx: &ServiceContext) {
    let Some(id) = ctx.default_browser().takeover.withdraw() else {
        return;
    };
    if let Err(error) = ctx.platform().notifier.close(id).await {
        tracing::debug!(%error, id, "cannot withdraw the takeover notification");
    }
}

/// The notification for `app` taking over; `name` is its display name.
pub(crate) fn notification(name: &str) -> Notification {
    Notification {
        summary: "Wye is no longer your default browser".to_owned(),
        body: format!("{name} took over, so links skip Wye."),
        actions: vec![
            (
                MAKE_DEFAULT_ACTION.to_owned(),
                "Make Wye Default".to_owned(),
            ),
            (KEEP_ACTION.to_owned(), format!("Keep {name}")),
        ],
        ..Notification::default()
    }
}

/// Answer the takeover notification's buttons.
pub(crate) fn spawn(ctx: &ServiceContext) -> JoinHandle<()> {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        let mut presses = ctx.platform().notifier.actions();
        loop {
            let press = match presses.recv().await {
                Ok(press) => press,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "missed notification buttons");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            };
            let action = press.action.as_str();
            if action != MAKE_DEFAULT_ACTION && action != KEEP_ACTION {
                continue;
            }
            let Some(app) = ctx.default_browser().takeover.answered(press.id) else {
                continue;
            };
            let answered = if action == MAKE_DEFAULT_ACTION {
                super::make_default(&ctx).await
            } else {
                super::keep(&ctx, Some(app)).await
            };
            if let Err(error) = answered {
                tracing::warn!(%error, action, "cannot answer the takeover notification");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> DesktopId {
        DesktopId::new(text).expect("valid")
    }

    #[test]
    fn only_losing_the_default_counts() {
        let watch = Watch::default();
        let other = Registration {
            is_default: false,
            current: Some(id("firefox.desktop")),
        };
        let wye = Registration {
            is_default: true,
            current: None,
        };
        assert_eq!(watch.saw(other.clone()), None, "never default yet");
        assert_eq!(watch.saw(wye.clone()), None);
        assert_eq!(watch.saw(wye), None);
        assert_eq!(
            watch.saw(other.clone()),
            Some(TookOver {
                app: Some(id("firefox.desktop"))
            })
        );
        assert_eq!(watch.saw(other), None, "one notification per takeover");
    }

    #[test]
    fn a_button_answers_only_its_notification() {
        let watch = Watch::default();
        watch.shown(7, id("firefox.desktop"));
        assert_eq!(watch.answered(8), None);
        assert_eq!(watch.answered(7), Some(id("firefox.desktop")));
        assert_eq!(watch.answered(7), None);
    }

    #[test]
    fn a_withdrawn_notification_answers_nothing() {
        let watch = Watch::default();
        assert_eq!(watch.withdraw(), None);
        watch.shown(7, id("firefox.desktop"));
        assert_eq!(watch.withdraw(), Some(7));
        assert_eq!(watch.answered(7), None, "ONB-11: a stale Keep button");
    }

    #[test]
    fn def03_the_notification_to_replace_is_kept_until_replaced() {
        let watch = Watch::default();
        assert_eq!(watch.on_screen(), None);
        watch.shown(7, id("firefox.desktop"));
        assert_eq!(watch.on_screen(), Some(7));
        assert_eq!(
            watch.answered(7),
            Some(id("firefox.desktop")),
            "still answers until a newer one is shown"
        );
        watch.shown(7, id("firefox.desktop"));
        watch.shown(9, id("brave.desktop"));
        assert_eq!(watch.answered(7), None);
        assert_eq!(watch.answered(9), Some(id("brave.desktop")));
    }

    #[test]
    fn the_notification_offers_both_answers() {
        let shown = notification("Firefox");
        assert!(shown.body.contains("Firefox took over"));
        assert_eq!(shown.actions[1].1, "Keep Firefox");
    }
}
