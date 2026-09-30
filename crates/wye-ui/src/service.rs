//! The D-Bus side of `wye-ui`: one tokio runtime on a background thread,
//! and the session-bus connection it serves and calls on.
//!
//! Qt objects never block on D-Bus. A backend calls [`request`] with its
//! `CxxQtThread`; the call runs on the runtime and its result is queued back
//! onto the Qt thread, where the backend updates its properties.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use cxx_qt::{CxxQtThread, Threading};
use tokio::runtime::Runtime;
use wye_api::Error;
use wye_api::proxy::Wye1Proxy;

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
    let proxy = Wye1Proxy::new(connection).await?;
    work(proxy).await
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
    use super::*;

    #[test]
    fn without_a_connection_the_call_fails_cleanly() {
        let runtime = runtime().expect("runtime");
        let result: Result<(), Error> = runtime.block_on(call(None, |_proxy| async { Ok(()) }));
        assert!(matches!(result, Err(Error::Failed(_))), "{result:?}");
    }
}
