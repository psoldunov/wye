//! Hands deliveries from the D-Bus thread to the Qt thread.
//!
//! The D-Bus objects are served before the name is requested, and before
//! QML has created the `App` that receives their calls: whatever arrives
//! first waits here, in order, until `App.attach()` (`crate::bridge::app`)
//! supplies the [`Sink`].

use std::sync::{Mutex, OnceLock, PoisonError};

use crate::route::Delivery;

/// Deliveries kept while no sink is attached. A bus activation brings one
/// or two; more means QML never started, and the caller gets an error.
const MAX_WAITING: usize = 32;

/// The receiving end, on the Qt thread.
pub trait Sink: Send {
    /// Queue `delivery` for the Qt thread.
    ///
    /// # Errors
    ///
    /// [`SinkClosed`] when the receiver is gone.
    fn deliver(&self, delivery: Delivery) -> Result<(), SinkClosed>;
}

/// The receiver was destroyed (the UI is quitting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the UI is shutting down")]
pub struct SinkClosed;

/// Why a delivery was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DispatchError {
    #[error(transparent)]
    Closed(#[from] SinkClosed),
    #[error("the UI is not ready: {MAX_WAITING} requests are already waiting")]
    Full,
    #[error("a receiver is already attached")]
    AlreadyAttached,
}

enum State {
    Waiting(Vec<Delivery>),
    Attached(Box<dyn Sink>),
}

/// Queue in front of the one [`Sink`].
pub struct Dispatcher {
    state: Mutex<State>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::Waiting(Vec::new())),
        }
    }
}

impl Dispatcher {
    /// Deliver now, or keep until a sink is attached.
    ///
    /// # Errors
    ///
    /// [`DispatchError::Closed`] when the sink is gone,
    /// [`DispatchError::Full`] when too many deliveries are waiting.
    pub fn send(&self, delivery: Delivery) -> Result<(), DispatchError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match &mut *state {
            State::Attached(sink) => Ok(sink.deliver(delivery)?),
            State::Waiting(waiting) if waiting.len() >= MAX_WAITING => Err(DispatchError::Full),
            State::Waiting(waiting) => {
                waiting.push(delivery);
                Ok(())
            }
        }
    }

    /// Attach `sink` and hand it everything waiting, in order.
    ///
    /// # Errors
    ///
    /// [`DispatchError::AlreadyAttached`] for a second sink, or
    /// [`DispatchError::Closed`] when `sink` refuses a waiting delivery (the
    /// rest are dropped with it).
    pub fn attach(&self, sink: Box<dyn Sink>) -> Result<(), DispatchError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let waiting = match &mut *state {
            State::Attached(_) => return Err(DispatchError::AlreadyAttached),
            State::Waiting(waiting) => std::mem::take(waiting),
        };
        let flushed = waiting
            .into_iter()
            .try_for_each(|delivery| sink.deliver(delivery));
        *state = State::Attached(sink);
        Ok(flushed?)
    }
}

/// The process's dispatcher: the D-Bus objects send into it, the `App`
/// QML creates attaches to it.
pub fn global() -> &'static Dispatcher {
    static DISPATCHER: OnceLock<Dispatcher> = OnceLock::new();
    DISPATCHER.get_or_init(Dispatcher::default)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    /// Records deliveries; closed when `open` is false.
    #[derive(Clone)]
    struct Recorder {
        seen: Arc<Mutex<Vec<Delivery>>>,
        open: bool,
    }

    impl Recorder {
        fn new(open: bool) -> Self {
            Self {
                seen: Arc::default(),
                open,
            }
        }

        fn seen(&self) -> Vec<Delivery> {
            self.seen.lock().expect("not poisoned").clone()
        }
    }

    impl Sink for Recorder {
        fn deliver(&self, delivery: Delivery) -> Result<(), SinkClosed> {
            if !self.open {
                return Err(SinkClosed);
            }
            self.seen.lock().expect("not poisoned").push(delivery);
            Ok(())
        }
    }

    #[test]
    fn early_deliveries_wait_and_arrive_in_order() {
        let dispatcher = Dispatcher::default();
        dispatcher.send(Delivery::Quit).expect("kept");
        let recorder = Recorder::new(true);
        dispatcher
            .attach(Box::new(recorder.clone()))
            .expect("attached");
        assert_eq!(recorder.seen(), vec![Delivery::Quit]);
        dispatcher.send(Delivery::Quit).expect("delivered");
        assert_eq!(recorder.seen().len(), 2);
    }

    #[test]
    fn a_second_sink_is_refused() {
        let dispatcher = Dispatcher::default();
        dispatcher
            .attach(Box::new(Recorder::new(true)))
            .expect("first");
        assert_eq!(
            dispatcher.attach(Box::new(Recorder::new(true))),
            Err(DispatchError::AlreadyAttached)
        );
    }

    #[test]
    fn the_queue_is_bounded() {
        let dispatcher = Dispatcher::default();
        for _ in 0..MAX_WAITING {
            dispatcher.send(Delivery::Quit).expect("kept");
        }
        assert_eq!(dispatcher.send(Delivery::Quit), Err(DispatchError::Full));
    }

    #[test]
    fn a_closed_sink_is_reported() {
        let dispatcher = Dispatcher::default();
        dispatcher
            .attach(Box::new(Recorder::new(false)))
            .expect("attached with nothing waiting");
        assert_eq!(
            dispatcher.send(Delivery::Quit),
            Err(DispatchError::Closed(SinkClosed))
        );
    }
}
