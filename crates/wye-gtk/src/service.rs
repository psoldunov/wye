//! The D-Bus side of `wye-gtk`: a tokio runtime on a background thread runs
//! every zbus future, and the GTK main context awaits the results.
//!
//! [`with_bus`] makes the session bus the windows' for as long as the
//! application runs: the runtime's handle and the connection `crate::host`
//! opened, kept on the main thread (every call below is made there). A
//! window calls [`request`]: the work is spawned on the runtime and a local
//! future on the main context awaits it, so GTK never blocks on D-Bus and
//! the result reaches the window on its own thread. A window that shows
//! what the service holds calls [`watch`] once instead of polling: the
//! service announces every property change (`PropertiesChanged`,
//! `docs/dbus-api.md`), and [`follow`] carries any other signal stream the
//! same way. Both return a [`Subscription`]: keep it in the object that
//! shows the data, and dropping that object stops it.
//!
//! The callbacks run on the main thread and are `'static`: capture widgets
//! and objects weakly (`glib::clone!(#[weak] …)`), so a window closed while
//! a call is in flight is simply not updated.
//!
//! The KDE host (crates/wye-ui/src/service.rs) offers the same calls over
//! Qt's thread; the delivery here is `GLib`'s own.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::{Rc, Weak};

use futures_lite::{Stream, StreamExt as _, stream};
use gtk::glib;
use tokio::runtime::{Handle, Runtime};
use wye_api::Error;
use wye_api::names::{BUS_NAME, INTERFACE, OBJECT_PATH};
use wye_api::proxy::Wye1Proxy;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::proxy::CacheProperties;

/// The D-Bus thread's name, as `top` and `gdb` show it.
const THREAD_NAME: &str = "wye-gtk-dbus";

/// A runtime for the D-Bus futures: one worker thread is plenty, every
/// call is I/O-bound and short. The caller owns it and shuts it down after
/// the application.
///
/// # Errors
///
/// When the runtime cannot be started (no threads left).
pub fn new_runtime() -> anyhow::Result<Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name(THREAD_NAME)
        .enable_all()
        .build()?)
}

/// What the windows reach the service through.
#[derive(Debug, Clone)]
struct Bus {
    runtime: Handle,
    connection: zbus::Connection,
}

thread_local! {
    /// The bus of [`with_bus`], on the main thread; `None` outside it (the
    /// self-test, which never touches the session bus).
    static BUS: RefCell<Option<Bus>> = const { RefCell::new(None) };
}

fn bus() -> Option<Bus> {
    BUS.with_borrow(Clone::clone)
}

/// Run `application` with `connection` as the windows' session bus, its
/// futures on `runtime`. Call it on the main thread.
pub fn with_bus<T>(
    runtime: Handle,
    connection: zbus::Connection,
    application: impl FnOnce() -> T,
) -> T {
    let previous = BUS.replace(Some(Bus {
        runtime,
        connection,
    }));
    let result = application();
    BUS.set(previous);
    result
}

/// Call the service with `work` and hand its result to `deliver` on the GTK
/// main context. Call it from the main thread.
///
/// Without a bus (the self-test, or a failed start) `deliver` gets
/// [`Error::Failed`].
pub fn request<W, F, R, D>(work: W, deliver: D)
where
    W: FnOnce(Wye1Proxy<'static>) -> F + Send + 'static,
    F: Future<Output = Result<R, Error>> + Send + 'static,
    R: Send + 'static,
    D: FnOnce(Result<R, Error>) + 'static,
{
    let task = bus().map(|bus| bus.runtime.spawn(call(bus.connection, work)));
    glib::spawn_future_local(async move {
        let result = match task {
            Some(task) => task.await.unwrap_or_else(|error| {
                Err(Error::failed(format!("the D-Bus call stopped: {error}")))
            }),
            None => Err(Error::failed("not connected to the session bus")),
        };
        deliver(result);
    });
}

/// Run `work` against the service on `connection`.
async fn call<W, F, R>(connection: zbus::Connection, work: W) -> Result<R, Error>
where
    W: FnOnce(Wye1Proxy<'static>) -> F,
    F: Future<Output = Result<R, Error>>,
{
    // A proxy per call, without the property cache: a cached proxy would
    // subscribe to `PropertiesChanged` and fetch every property (`GetAll`)
    // before the first read, only to drop it all again.
    let proxy = Wye1Proxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    work(proxy).await
}

/// A stream followed for an object on the main thread. Dropping it stops
/// both ends: the subscription on the D-Bus thread and the local future
/// that hands items to the callback.
#[must_use = "dropping the subscription stops it"]
#[derive(Default)]
pub struct Subscription {
    remote: Option<tokio::task::AbortHandle>,
    local: Option<glib::JoinHandle<()>>,
}

impl Subscription {
    /// Whether anything is followed: false without a bus (the self-test).
    pub const fn is_active(&self) -> bool {
        self.remote.is_some()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(remote) = self.remote.take() {
            remote.abort();
        }
        if let Some(local) = self.local.take() {
            local.abort();
        }
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .field("active", &self.is_active())
            .finish()
    }
}

/// Follow a signal stream until the returned [`Subscription`] is dropped:
/// `open` subscribes on the D-Bus thread, and each item reaches `deliver` on
/// the main context, in order. It also stops when the stream ends (the bus
/// connection closed). Without a bus (the self-test) there is nothing to
/// follow. Call it from the main thread.
pub fn follow<O, F, S, D>(open: O, deliver: D) -> Subscription
where
    O: FnOnce(zbus::Connection) -> F + Send + 'static,
    F: Future<Output = Result<S, Error>> + Send + 'static,
    S: Stream + Unpin + Send + 'static,
    S::Item: Send + 'static,
    D: Fn(S::Item) + 'static,
{
    let Some(bus) = bus() else {
        return Subscription::default();
    };
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let remote = bus.runtime.spawn(async move {
        let mut items = match open(bus.connection).await {
            Ok(items) => items,
            Err(error) => {
                tracing::info!(%error, "cannot follow the service");
                return;
            }
        };
        while let Some(item) = items.next().await {
            if sender.send(item).is_err() {
                // The main-thread end is gone: nobody left to tell.
                break;
            }
        }
    });
    let local = glib::spawn_future_local(async move {
        while let Some(item) = receiver.recv().await {
            deliver(item);
        }
    });
    Subscription {
        remote: Some(remote.abort_handle()),
        local: Some(local),
    }
}

/// Why a watched window should read the service again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A watched property moved, or the subscription just started.
    Moved,
    /// A service took the bus name: a new one starts its revisions over at
    /// 1, so read everything, not only what moved since the last read.
    Restarted,
}

impl Change {
    /// One change standing for `self` and a later `next`: a restart asks for
    /// the larger read, which covers a move too.
    const fn and(self, next: Self) -> Self {
        match (self, next) {
            (Self::Moved, Self::Moved) => Self::Moved,
            _ => Self::Restarted,
        }
    }
}

/// Call `changed` on the main context whenever one of `properties` of
/// `dev.soldunov.wye1` changes ([`Change::Moved`]), when a service takes the
/// bus name ([`Change::Restarted`]), and once as soon as the subscription
/// stands, for anything that changed while it was being set up. The changes
/// that arrive before the main loop is idle again (one configuration change
/// moves `ConfigRevision`, `Status` and `Tray`) are one call. `changed`
/// reads what it needs itself.
///
/// The service leaving the bus (TRAY-17 "Quit Wye") is not a change: a read
/// then would start it again through D-Bus activation.
pub fn watch<D>(properties: &'static [&'static str], changed: D) -> Subscription
where
    D: Fn(Change) + 'static,
{
    let batch = Rc::new(Batch {
        pending: Cell::new(None),
        changed,
    });
    follow(
        move |connection| changes(connection, properties),
        move |change| Batch::add(&batch, change),
    )
}

/// The changes waiting for the main loop's next idle moment.
struct Batch<D> {
    pending: Cell<Option<Change>>,
    changed: D,
}

impl<D: Fn(Change) + 'static> Batch<D> {
    fn add(this: &Rc<Self>, change: Change) {
        let waiting = this.pending.get();
        this.pending
            .set(Some(waiting.map_or(change, |earlier| earlier.and(change))));
        if waiting.is_some() {
            return;
        }
        let batch: Weak<Self> = Rc::downgrade(this);
        let context = glib::MainContext::ref_thread_default();
        context.spawn_local_with_priority(glib::Priority::DEFAULT_IDLE, async move {
            let Some(batch) = batch.upgrade() else { return };
            if let Some(change) = batch.pending.take() {
                (batch.changed)(change);
            }
        });
    }
}

/// The stream behind [`watch`]: one item per change worth a reload.
async fn changes(
    connection: zbus::Connection,
    properties: &'static [&'static str],
) -> Result<impl Stream<Item = Change> + Unpin + Send, Error> {
    let moved = PropertiesProxy::builder(&connection)
        .destination(BUS_NAME)?
        .path(OBJECT_PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?
        .receive_properties_changed()
        .await?
        .filter_map(move |signal| {
            let args = signal.args().ok()?;
            let watched = |name: &&str| {
                args.changed_properties.contains_key(name)
                    || args.invalidated_properties.contains(name)
            };
            (args.interface_name.as_str() == INTERFACE && properties.iter().any(watched))
                .then_some(Change::Moved)
        });
    let restarted = DBusProxy::new(&connection)
        .await?
        .receive_name_owner_changed_with_args(&[(0, BUS_NAME)])
        .await?
        .filter_map(|signal| {
            signal
                .args()
                .ok()?
                .new_owner()
                .is_some()
                .then_some(Change::Restarted)
        });
    Ok(stream::once(Change::Moved).chain(moved.or(restarted)))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_bus::PrivateBus;

    /// How long a test waits for an item that should not come.
    const QUIET: Duration = Duration::from_millis(300);

    /// Two properties of the service's interface.
    struct FakeService {
        config_revision: u64,
        tray: String,
    }

    #[zbus::interface(name = "dev.soldunov.wye1")]
    impl FakeService {
        #[zbus(property)]
        fn config_revision(&self) -> u64 {
            self.config_revision
        }

        #[zbus(property)]
        fn tray(&self) -> String {
            self.tray.clone()
        }
    }

    async fn next(items: &mut (impl Stream<Item = Change> + Unpin)) -> Option<Change> {
        tokio::time::timeout(QUIET, items.next())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    async fn set_06_watched_changes_and_restarts_are_announced() {
        let Some(bus) = PrivateBus::start() else {
            return;
        };
        let service = bus
            .builder()
            .name(BUS_NAME)
            .expect("name")
            .serve_at(
                OBJECT_PATH,
                FakeService {
                    config_revision: 1,
                    tray: String::new(),
                },
            )
            .expect("served")
            .build()
            .await
            .expect("service");
        let client = bus.builder().build().await.expect("client");
        let mut items = changes(client, &["ConfigRevision"])
            .await
            .expect("subscribed");

        // Once as soon as the subscription stands.
        assert_eq!(next(&mut items).await, Some(Change::Moved));

        let iface = service
            .object_server()
            .interface::<_, FakeService>(OBJECT_PATH)
            .await
            .expect("interface");
        iface.get_mut().await.config_revision = 2;
        let emitter = iface.signal_emitter();
        iface
            .get()
            .await
            .config_revision_changed(emitter)
            .await
            .expect("emitted");
        assert_eq!(next(&mut items).await, Some(Change::Moved), "watched");

        iface.get_mut().await.tray = "{}".to_owned();
        iface
            .get()
            .await
            .tray_changed(iface.signal_emitter())
            .await
            .expect("emitted");
        assert_eq!(next(&mut items).await, None, "not a watched property");

        // TRAY-17: the service quits. Reading now would start it again.
        drop(iface);
        service.close().await.expect("the service leaves");
        assert_eq!(next(&mut items).await, None, "leaving is not a change");

        let _again = bus
            .builder()
            .name(BUS_NAME)
            .expect("name")
            .build()
            .await
            .expect("a new service");
        assert_eq!(next(&mut items).await, Some(Change::Restarted));
    }

    #[test]
    fn a_burst_is_one_change_and_a_restart_wins() {
        assert_eq!(Change::Moved.and(Change::Moved), Change::Moved);
        assert_eq!(Change::Moved.and(Change::Restarted), Change::Restarted);
        assert_eq!(Change::Restarted.and(Change::Moved), Change::Restarted);
    }

    #[test]
    fn changes_before_the_loop_is_idle_are_one_call() {
        let context = glib::MainContext::new();
        let calls = Rc::new(RefCell::new(Vec::new()));
        context
            .with_thread_default(|| {
                let seen = Rc::clone(&calls);
                let batch = Rc::new(Batch {
                    pending: Cell::new(None),
                    changed: move |change| seen.borrow_mut().push(change),
                });
                Batch::add(&batch, Change::Moved);
                Batch::add(&batch, Change::Moved);
                Batch::add(&batch, Change::Restarted);
                while context.iteration(false) {}
                Batch::add(&batch, Change::Moved);
                while context.iteration(false) {}
            })
            .expect("the test owns its context");
        assert_eq!(*calls.borrow(), [Change::Restarted, Change::Moved]);
    }

    #[test]
    fn without_a_bus_a_request_fails_and_nothing_is_followed() {
        let context = glib::MainContext::new();
        let answer = context.block_on(async {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            request(
                |proxy| async move { proxy.picker_cancelled("1").await },
                move |result| {
                    let _ = sender.send(result);
                },
            );
            receiver.await.expect("delivered")
        });
        assert!(matches!(answer, Err(Error::Failed(_))), "{answer:?}");
        assert!(!watch(&["ConfigRevision"], |_| {}).is_active());
    }
}
