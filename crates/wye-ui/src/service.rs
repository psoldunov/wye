//! The D-Bus side of `wye-ui`: one tokio runtime on a background thread,
//! and the session-bus connection it serves and calls on.
//!
//! Qt objects never block on D-Bus. A backend calls [`request`] with its
//! `CxxQtThread`; the call runs on the runtime and its result is queued back
//! onto the Qt thread, where the backend updates its properties. A window
//! that shows what the service holds calls [`watch`] once instead of
//! polling: the service announces every property change
//! (`PropertiesChanged`, `docs/dbus-api.md`), and [`follow`] carries any
//! other signal stream the same way.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use cxx_qt::{CxxQtThread, Threading};
use futures_lite::{Stream, StreamExt as _, stream};
use tokio::runtime::Runtime;
use wye_api::Error;
use wye_api::names::{BUS_NAME, INTERFACE, OBJECT_PATH};
use wye_api::proxy::Wye1Proxy;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::proxy::CacheProperties;

/// One worker is plenty: every call is I/O-bound and short.
const WORKER_THREADS: usize = 1;

/// The thread names, as `top` and `gdb` show them.
const THREAD_NAME: &str = "wye-ui-dbus";

/// The runtime every D-Bus future of the process runs on.
///
/// # Errors
///
/// When the runtime cannot be started (no threads left).
pub fn runtime() -> anyhow::Result<&'static Runtime> {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKER_THREADS)
        .thread_name(THREAD_NAME)
        .enable_all()
        .build()?;
    // Another thread may have won the race; its runtime is used and this one
    // is dropped unused.
    Ok(RUNTIME.get_or_init(|| runtime))
}

static CONNECTION: OnceLock<zbus::Connection> = OnceLock::new();

/// Remember the session-bus connection `crate::host` opened.
pub fn set_connection(connection: zbus::Connection) {
    if CONNECTION.set(connection).is_err() {
        tracing::warn!("the session-bus connection was already set");
    }
}

/// Call the service with `work` and hand its result to `deliver` on the Qt
/// thread that owns the object `thread` came from.
///
/// Without a connection (the self-test, or a failed start) `deliver` gets
/// [`Error::Failed`]. When the object is gone by the time the result is
/// ready, the result is dropped and logged.
pub fn request<T, W, F, R, D>(thread: CxxQtThread<T>, work: W, deliver: D)
where
    T: Threading + 'static,
    W: FnOnce(Wye1Proxy<'static>) -> F + Send + 'static,
    F: Future<Output = Result<R, Error>> + Send + 'static,
    R: Send + 'static,
    D: FnOnce(Pin<&mut T>, Result<R, Error>) + Send + 'static,
{
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            let message = format!("no D-Bus runtime: {error}");
            queue(&thread, deliver, Err(Error::failed(message)));
            return;
        }
    };
    runtime.spawn(async move {
        let result = call(CONNECTION.get(), work).await;
        queue(&thread, deliver, result);
    });
}

/// Run `work` against the service on `connection`.
async fn call<W, F, R>(connection: Option<&zbus::Connection>, work: W) -> Result<R, Error>
where
    W: FnOnce(Wye1Proxy<'static>) -> F,
    F: Future<Output = Result<R, Error>>,
{
    let Some(connection) = connection else {
        return Err(Error::failed("not connected to the session bus"));
    };
    // A proxy per call, without the property cache: a cached proxy would
    // subscribe to `PropertiesChanged` and fetch every property (`GetAll`)
    // before the first read, only to drop it all again.
    let proxy = Wye1Proxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    work(proxy).await
}

/// Follow a signal stream for as long as the object `thread` came from
/// lives: `open` subscribes on the D-Bus thread, and each item reaches
/// `deliver` on the Qt thread, in order. It stops when the object is gone or
/// the stream ends (the bus connection closed). Without a connection (the
/// self-test) there is nothing to follow.
pub fn follow<T, O, F, S, D>(thread: CxxQtThread<T>, open: O, deliver: D)
where
    T: Threading + 'static,
    O: FnOnce(zbus::Connection) -> F + Send + 'static,
    F: Future<Output = Result<S, Error>> + Send + 'static,
    S: Stream + Unpin + Send + 'static,
    S::Item: Send + 'static,
    D: Fn(Pin<&mut T>, S::Item) + Send + Sync + 'static,
{
    let Some(connection) = CONNECTION.get().cloned() else {
        return;
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::warn!(%error, "no D-Bus runtime to follow the service on");
            return;
        }
    };
    let deliver = Arc::new(deliver);
    runtime.spawn(async move {
        let mut items = match open(connection).await {
            Ok(items) => items,
            Err(error) => {
                tracing::info!(%error, "cannot follow the service");
                return;
            }
        };
        while let Some(item) = items.next().await {
            let deliver = Arc::clone(&deliver);
            if thread.queue(move |object| deliver(object, item)).is_err() {
                // The object is gone: nobody left to tell.
                break;
            }
        }
    });
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

/// Call `changed` on the Qt thread whenever one of `properties` of
/// `dev.soldunov.wye1` changes ([`Change::Moved`]), when a service takes the
/// bus name ([`Change::Restarted`]), and once as soon as the subscription
/// stands, for anything that changed while it was being set up. A burst of
/// changes (one configuration change moves `ConfigRevision`, `Status` and
/// `Tray`) arrives as one call. `changed` reads what it needs itself.
///
/// The service leaving the bus (TRAY-17 "Quit Wye") is not a change: a read
/// then would start it again through D-Bus activation.
pub fn watch<T, D>(thread: CxxQtThread<T>, properties: &'static [&'static str], changed: D)
where
    T: Threading + 'static,
    D: Fn(Pin<&mut T>, Change) + Send + Sync + 'static,
{
    let queued = Arc::new(AtomicBool::new(false));
    let delivered = Arc::clone(&queued);
    follow(
        thread,
        move |connection| changes(connection, properties, queued),
        move |object, change| {
            // Cleared before reading, so a change during the read is not lost.
            delivered.store(false, Ordering::SeqCst);
            changed(object, change);
        },
    );
}

/// The stream behind [`watch`]: one item per change worth a reload. A
/// [`Change::Moved`] is dropped while the last item still waits on the Qt
/// thread; a [`Change::Restarted`] never is, since the read it asks for is a
/// different one.
async fn changes(
    connection: zbus::Connection,
    properties: &'static [&'static str],
    queued: Arc<AtomicBool>,
) -> Result<impl Stream<Item = Change> + Unpin + Send, Error> {
    let changed = PropertiesProxy::builder(&connection)
        .destination(BUS_NAME)?
        .path(OBJECT_PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?
        .receive_properties_changed()
        .await?
        .filter_map(move |signal| {
            let args = signal.args().ok()?;
            let ours = args.interface_name.as_str() == INTERFACE
                && properties.iter().any(|name| {
                    args.changed_properties.contains_key(name)
                        || args.invalidated_properties.contains(name)
                });
            ours.then_some(Change::Moved)
        });
    let owners = DBusProxy::new(&connection)
        .await?
        .receive_name_owner_changed_with_args(&[(0, BUS_NAME)])
        .await?
        .filter_map(|signal| {
            let args = signal.args().ok()?;
            args.new_owner().is_some().then_some(Change::Restarted)
        });
    Ok(stream::once(Change::Moved)
        .chain(changed.or(owners))
        .filter(move |change| {
            let waiting = queued.swap(true, Ordering::SeqCst);
            !waiting || *change == Change::Restarted
        }))
}

fn queue<T, R, D>(thread: &CxxQtThread<T>, deliver: D, result: Result<R, Error>)
where
    T: Threading + 'static,
    R: Send + 'static,
    D: FnOnce(Pin<&mut T>, Result<R, Error>) + Send + 'static,
{
    if let Err(error) = thread.queue(move |object| deliver(object, result)) {
        tracing::debug!("dropped a service result: {error}");
    }
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
    async fn set_06_a_watched_property_change_is_announced_once() {
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
        let queued = Arc::new(AtomicBool::new(false));
        let mut items = changes(client, &["ConfigRevision"], Arc::clone(&queued))
            .await
            .expect("subscribed");

        // Once as soon as the subscription stands.
        assert_eq!(next(&mut items).await, Some(Change::Moved));
        queued.store(false, Ordering::SeqCst);

        let iface = service
            .object_server()
            .interface::<_, FakeService>(OBJECT_PATH)
            .await
            .expect("interface");
        iface.get_mut().await.config_revision = 2;
        iface
            .get()
            .await
            .config_revision_changed(iface.signal_emitter())
            .await
            .expect("emitted");
        assert_eq!(
            next(&mut items).await,
            Some(Change::Moved),
            "a watched property"
        );
        queued.store(false, Ordering::SeqCst);

        iface.get_mut().await.tray = "{}".to_owned();
        iface
            .get()
            .await
            .tray_changed(iface.signal_emitter())
            .await
            .expect("emitted");
        assert_eq!(next(&mut items).await, None, "not a watched property");

        // While the last change still waits on the Qt thread, more are one.
        queued.store(true, Ordering::SeqCst);
        iface.get_mut().await.config_revision = 3;
        iface
            .get()
            .await
            .config_revision_changed(iface.signal_emitter())
            .await
            .expect("emitted");
        assert_eq!(next(&mut items).await, None, "coalesced");
        queued.store(false, Ordering::SeqCst);

        // TRAY-17: the service quits. Reading now would start it again.
        drop(iface);
        service.close().await.expect("the service leaves");
        assert_eq!(next(&mut items).await, None, "leaving is not a change");

        // A new service: read everything, even while a move still waits.
        queued.store(true, Ordering::SeqCst);
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
    fn without_a_connection_the_call_fails_cleanly() {
        let runtime = runtime().expect("runtime");
        let result: Result<(), Error> = runtime.block_on(call(None, |_proxy| async { Ok(()) }));
        assert!(matches!(result, Err(Error::Failed(_))), "{result:?}");
    }
}
