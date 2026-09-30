//! The clipboard on Wayland through data control (EXT-12, TRAY-10, risk 9):
//! `ext-data-control-v1`, else `zwlr-data-control-v1`.
//!
//! A worker thread owns the connection. It reads every new selection that
//! offers plain text (and no password-manager hint or image), keeps the
//! latest text for [`ClipboardProvider::read`], announces other clients'
//! text on the watch channel, and serves Wye's own writes from a data source
//! it keeps alive until another client takes the clipboard. Reads from the
//! other client's pipe run as tasks, so the worker keeps answering the
//! compositor (and its own source) meanwhile.

mod dispatch;
mod protocol;

use std::collections::HashMap;
use std::fs::File;
use std::io::Write as _;
use std::os::fd::{AsFd as _, OwnedFd};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncReadExt as _, Interest};
use tokio::sync::{broadcast, mpsc, oneshot};
use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, EventQueue, QueueHandle};

use self::protocol::{Device, Manager, Offer, Source};
use super::{ClipboardCapabilities, ClipboardProvider, PlatformError, TEXT_MIMES, is_blocked};

/// How long starting the worker may take.
const START_TIMEOUT: Duration = Duration::from_secs(2);
/// How long another client may take to hand over its text.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// How long a write may take to be set up.
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);
/// Longest text read; anything longer is not a link.
const MAX_TEXT: u64 = 64 * 1024;
/// Changes the watchers may fall behind by.
const BUFFER: usize = 16;
const SEAT_VERSION: u32 = 1;
const MANAGER_VERSION: u32 = 1;

/// What the worker shares with the provider.
#[derive(Debug)]
struct Shared {
    current: Mutex<Option<String>>,
    changes: broadcast::Sender<String>,
}

/// A request to the worker.
enum Command {
    Write(String, oneshot::Sender<()>),
}

/// The clipboard over data control.
#[derive(Debug)]
pub struct DataControlClipboard {
    shared: Arc<Shared>,
    commands: mpsc::UnboundedSender<Command>,
    mechanism: &'static str,
}

impl DataControlClipboard {
    /// Connect to the Wayland display and start watching.
    ///
    /// # Errors
    ///
    /// `Unavailable` without a display or without a data-control manager.
    pub async fn start() -> Result<Self, PlatformError> {
        let shared = Arc::new(Shared {
            current: Mutex::new(None),
            changes: broadcast::Sender::new(BUFFER),
        });
        let (commands, receiver) = mpsc::unbounded_channel();
        let (ready, started) = oneshot::channel();
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("wye-clipboard".to_owned())
            .spawn(move || work(&worker, receiver, ready))
            .map_err(|error| {
                PlatformError::Failed(format!("cannot start the clipboard: {error}"))
            })?;
        let mechanism = tokio::time::timeout(START_TIMEOUT, started)
            .await
            .map_err(|_| PlatformError::Timeout(START_TIMEOUT))?
            .map_err(|_| PlatformError::Failed("the clipboard worker stopped".to_owned()))??;
        Ok(Self {
            shared,
            commands,
            mechanism,
        })
    }
}

#[async_trait]
impl ClipboardProvider for DataControlClipboard {
    async fn read(&self) -> Result<Option<String>, PlatformError> {
        Ok(self
            .shared
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    async fn write(&self, text: &str) -> Result<(), PlatformError> {
        let (done, written) = oneshot::channel();
        self.commands
            .send(Command::Write(text.to_owned(), done))
            .map_err(|_| PlatformError::Failed("the clipboard worker stopped".to_owned()))?;
        tokio::time::timeout(WRITE_TIMEOUT, written)
            .await
            .map_err(|_| PlatformError::Timeout(WRITE_TIMEOUT))?
            .map_err(|_| PlatformError::Failed("the clipboard worker stopped".to_owned()))
    }

    fn watch(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.shared.changes.subscribe())
    }

    fn capabilities(&self) -> ClipboardCapabilities {
        ClipboardCapabilities {
            read: Some(self.mechanism),
            watch: Some(self.mechanism),
            write: Some(self.mechanism),
        }
    }
}

/// The worker thread: its own single-threaded runtime around the session.
fn work(
    shared: &Arc<Shared>,
    commands: mpsc::UnboundedReceiver<Command>,
    ready: oneshot::Sender<Result<&'static str, PlatformError>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(PlatformError::Failed(error.to_string())));
            return;
        }
    };
    runtime.block_on(async move {
        let session = match Session::connect().await {
            Ok(session) => session,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(session.manager.mechanism()));
        if let Err(error) = session.run(shared, commands).await {
            tracing::warn!(%error, "the clipboard connection ended");
        }
    });
}

/// A text read from another client's selection.
struct Read {
    generation: u64,
    text: Option<String>,
}

/// The connection and its objects.
struct Session {
    connection: Connection,
    queue: EventQueue<Clip>,
    handle: QueueHandle<Clip>,
    readable: AsyncFd<OwnedFd>,
    clip: Clip,
    manager: Manager,
    device: Device,
}

impl Session {
    async fn connect() -> Result<Self, PlatformError> {
        let connection = Connection::connect_to_env()
            .map_err(|error| PlatformError::Unavailable(format!("no Wayland display: {error}")))?;
        let fd = connection
            .backend()
            .poll_fd()
            .try_clone_to_owned()
            .map_err(|error| failed(&error))?;
        let readable =
            AsyncFd::with_interest(fd, Interest::READABLE).map_err(|error| failed(&error))?;
        let queue = connection.new_event_queue();
        let handle = queue.handle();
        let display = connection.display();
        let registry = display.get_registry(&handle, ());
        display.sync(&handle, ());
        let mut session = PartialSession {
            connection,
            queue,
            handle,
            readable,
            clip: Clip::default(),
        };
        session.pump_until(|clip| clip.synced).await?;
        let globals = std::mem::take(&mut session.clip.globals);
        let find = |interface: &str| {
            globals
                .iter()
                .find(|(_, name, _)| name == interface)
                .map(|(id, _, version)| (*id, *version))
        };
        let handle = &session.handle;
        let (seat_name, seat_version) =
            find("wl_seat").ok_or_else(|| PlatformError::Unavailable("no seat".to_owned()))?;
        let seat: WlSeat = registry.bind(seat_name, seat_version.min(SEAT_VERSION), handle, ());
        let manager = if let Some((name, version)) = find(protocol::EXT_MANAGER) {
            Manager::Ext(registry.bind(name, version.min(MANAGER_VERSION), handle, ()))
        } else if let Some((name, version)) = find(protocol::WLR_MANAGER) {
            Manager::Wlr(registry.bind(name, version.min(MANAGER_VERSION), handle, ()))
        } else {
            return Err(PlatformError::Unavailable(
                "the compositor offers no data-control protocol".to_owned(),
            ));
        };
        let device = manager.device(&seat, handle);
        Ok(Self {
            connection: session.connection,
            queue: session.queue,
            handle: session.handle,
            readable: session.readable,
            clip: session.clip,
            manager,
            device,
        })
    }

    /// Serve events, reads and commands until the connection ends.
    async fn run(
        mut self,
        shared: &Arc<Shared>,
        mut commands: mpsc::UnboundedReceiver<Command>,
    ) -> Result<(), PlatformError> {
        let (read_done, mut done_reads) = mpsc::unbounded_channel::<Read>();
        loop {
            self.queue
                .dispatch_pending(&mut self.clip)
                .map_err(|error| failed(&error))?;
            if self.clip.finished {
                return Err(PlatformError::Failed(
                    "the data device went away".to_owned(),
                ));
            }
            self.start_reads(&read_done);
            publish(shared, self.clip.updates.drain(..));
            self.connection.flush().map_err(|error| failed(&error))?;
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };
            tokio::select! {
                ready = self.readable.readable() => {
                    let mut ready = ready.map_err(|error| failed(&error))?;
                    match guard.read() {
                        Ok(_) => {}
                        Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            ready.clear_ready();
                        }
                        Err(error) => return Err(failed(&error)),
                    }
                }
                command = commands.recv() => {
                    drop(guard);
                    let Some(Command::Write(text, done)) = command else { return Ok(()) };
                    self.clip.write(&self.manager, &self.device, &self.handle, text);
                    let _ = done.send(());
                }
                read = done_reads.recv() => {
                    drop(guard);
                    if let Some(read) = read.filter(|read| read.generation == self.clip.generation) {
                        self.clip.updates.push(Update { text: read.text, foreign: true });
                    }
                }
            }
        }
    }

    fn start_reads(&mut self, reads_to: &mpsc::UnboundedSender<Read>) {
        for (generation, fd) in self.clip.reads.drain(..) {
            let reads_to = reads_to.clone();
            tokio::spawn(async move {
                let text = read_pipe(fd).await;
                let _ = reads_to.send(Read { generation, text });
            });
        }
    }
}

/// The connection before the manager and device exist.
struct PartialSession {
    connection: Connection,
    queue: EventQueue<Clip>,
    handle: QueueHandle<Clip>,
    readable: AsyncFd<OwnedFd>,
    clip: Clip,
}

impl PartialSession {
    async fn pump_until(&mut self, done: impl Fn(&Clip) -> bool) -> Result<(), PlatformError> {
        loop {
            self.queue
                .dispatch_pending(&mut self.clip)
                .map_err(|error| failed(&error))?;
            if done(&self.clip) {
                return Ok(());
            }
            self.connection.flush().map_err(|error| failed(&error))?;
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };
            let mut ready = self
                .readable
                .readable()
                .await
                .map_err(|error| failed(&error))?;
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    ready.clear_ready();
                }
                Err(error) => return Err(failed(&error)),
            }
        }
    }
}

/// Publish what the clipboard now holds; announce other clients' text.
fn publish(shared: &Shared, updates: impl Iterator<Item = Update>) {
    for update in updates {
        let text = update.text.as_deref().and_then(super::single_line_text);
        shared
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone_from(&text);
        if update.foreign
            && let Some(text) = text
        {
            // Nobody watching is fine: rewrites may be off.
            let _ = shared.changes.send(text);
        }
    }
}

/// Read another client's text, up to [`MAX_TEXT`], within [`READ_TIMEOUT`].
async fn read_pipe(fd: OwnedFd) -> Option<String> {
    let reading = async {
        let pipe = tokio::net::unix::pipe::Receiver::from_owned_fd(fd).ok()?;
        let mut bytes = Vec::new();
        pipe.take(MAX_TEXT).read_to_end(&mut bytes).await.ok()?;
        String::from_utf8(bytes).ok()
    };
    tokio::time::timeout(READ_TIMEOUT, reading)
        .await
        .ok()
        .flatten()
}

/// A change of what the clipboard holds.
#[derive(Debug)]
struct Update {
    text: Option<String>,
    /// Another client put it there (Wye's own writes are not announced).
    foreign: bool,
}

/// What the event handlers collect.
#[derive(Debug, Default)]
pub struct Clip {
    globals: Vec<(u32, String, u32)>,
    synced: bool,
    finished: bool,
    /// MIME types each live offer announced.
    mimes: HashMap<ObjectId, Vec<String>>,
    selection: Option<Offer>,
    /// Bumps with every selection; a read for an older one is dropped.
    generation: u64,
    /// Selections Wye set that the compositor has not announced yet.
    own_pending: u32,
    /// Wye's data source and its text.
    source: Option<(Source, String)>,
    /// Pipes to read, with the selection they belong to.
    reads: Vec<(u64, OwnedFd)>,
    updates: Vec<Update>,
}

impl Clip {
    fn data_offer(&mut self, offer: &Offer) {
        self.mimes.insert(offer.key(), Vec::new());
    }

    fn mime(&mut self, offer: &Offer, mime: String) {
        self.mimes.entry(offer.key()).or_default().push(mime);
    }

    fn forget(&mut self, offer: &Offer) {
        self.mimes.remove(&offer.key());
        offer.destroy();
    }

    fn selection(&mut self, offer: Option<Offer>) {
        self.generation += 1;
        if let Some(old) = self.selection.take() {
            self.forget(&old);
        }
        self.selection.clone_from(&offer);
        let Some(offer) = offer else {
            self.updates.push(Update {
                text: None,
                foreign: true,
            });
            return;
        };
        if self.own_pending > 0 {
            self.own_pending -= 1;
            let text = self.source.as_ref().map(|(_, text)| text.clone());
            self.updates.push(Update {
                text,
                foreign: false,
            });
            return;
        }
        let mimes = self.mimes.get(&offer.key()).cloned().unwrap_or_default();
        let Some(mime) = TEXT_MIMES
            .iter()
            .find(|wanted| mimes.iter().any(|mime| mime == *wanted))
            .filter(|_| !is_blocked(&mimes))
        else {
            // Secrets, images and non-text are never read (EXT-12).
            self.updates.push(Update {
                text: None,
                foreign: false,
            });
            return;
        };
        match std::io::pipe() {
            Ok((reader, writer)) => {
                offer.receive((*mime).to_owned(), writer.as_fd());
                drop(writer);
                self.reads.push((self.generation, OwnedFd::from(reader)));
            }
            Err(error) => tracing::warn!(%error, "cannot read the clipboard"),
        }
    }

    fn write(
        &mut self,
        manager: &Manager,
        device: &Device,
        handle: &QueueHandle<Self>,
        text: String,
    ) {
        let source = manager.source(handle);
        for mime in TEXT_MIMES {
            source.offer(mime);
        }
        device.set_selection(&source);
        if let Some((old, _)) = self.source.replace((source, text)) {
            old.destroy();
        }
        self.own_pending += 1;
    }

    fn send(&mut self, source: &Source, fd: OwnedFd) {
        let Some((_, text)) = self.source.as_ref().filter(|(ours, _)| ours == source) else {
            return;
        };
        if let Err(error) = File::from(fd).write_all(text.as_bytes()) {
            tracing::debug!(%error, "a clipboard reader went away");
        }
    }

    fn cancelled(&mut self, source: &Source) {
        if self.source.as_ref().is_some_and(|(ours, _)| ours == source) {
            self.source = None;
        }
        source.destroy();
    }
}

fn failed(error: &dyn std::fmt::Display) -> PlatformError {
    PlatformError::Failed(format!("Wayland clipboard: {error}"))
}
