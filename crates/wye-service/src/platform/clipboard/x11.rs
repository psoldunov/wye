//! The clipboard on X11 (EXT-12, TRAY-10): XFIXES tells when the
//! `CLIPBOARD` selection changes owner; Wye then asks the owner for its
//! targets and, when they are plain text without a password-manager hint or
//! an image, for the text. Writing makes Wye the owner and answers
//! `SelectionRequest`s until another client takes the clipboard.
//!
//! A worker thread owns the connection; x11rb's connection is blocking, and
//! the worker waits on its socket and on write commands together.

use std::os::fd::{AsFd as _, OwnedFd};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::sync::{broadcast, mpsc, oneshot};
use x11rb::CURRENT_TIME;
use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{
    ClipboardCapabilities, ClipboardProvider, PASSWORD_HINT, PlatformError, single_line_text,
};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "x11-xfixes";
/// How long starting the worker may take.
const START_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a write may take to be set up.
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);
/// Longest text read, in 32-bit units for `GetProperty`.
const MAX_TEXT_UNITS: u32 = 16 * 1024;
const BUFFER: usize = 16;

/// Atoms the worker uses.
#[derive(Debug, Clone, Copy)]
struct Atoms {
    clipboard: Atom,
    targets: Atom,
    utf8: Atom,
    text: Atom,
    property: Atom,
    password_hint: Atom,
}

/// What the worker shares with the provider.
#[derive(Debug)]
struct Shared {
    current: Mutex<Option<String>>,
    changes: broadcast::Sender<String>,
}

/// The clipboard over X11.
#[derive(Debug)]
pub struct XfixesClipboard {
    shared: Arc<Shared>,
    commands: mpsc::UnboundedSender<(String, oneshot::Sender<()>)>,
}

impl XfixesClipboard {
    /// Connect to the X display and start watching.
    ///
    /// # Errors
    ///
    /// `Unavailable` without a display or XFIXES.
    pub async fn start() -> Result<Self, PlatformError> {
        let shared = Arc::new(Shared {
            current: Mutex::new(None),
            changes: broadcast::Sender::new(BUFFER),
        });
        let (commands, receiver) = mpsc::unbounded_channel();
        let (ready, started) = oneshot::channel();
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("wye-x11-clipboard".to_owned())
            .spawn(move || work(&worker, receiver, ready))
            .map_err(|error| {
                PlatformError::Failed(format!("cannot start the clipboard: {error}"))
            })?;
        tokio::time::timeout(START_TIMEOUT, started)
            .await
            .map_err(|_| PlatformError::Timeout(START_TIMEOUT))?
            .map_err(|_| PlatformError::Failed("the clipboard worker stopped".to_owned()))??;
        Ok(Self { shared, commands })
    }
}

#[async_trait]
impl ClipboardProvider for XfixesClipboard {
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
            .send((text.to_owned(), done))
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
            read: Some(MECHANISM),
            watch: Some(MECHANISM),
            write: Some(MECHANISM),
        }
    }
}

fn work(
    shared: &Arc<Shared>,
    commands: mpsc::UnboundedReceiver<(String, oneshot::Sender<()>)>,
    ready: oneshot::Sender<Result<(), PlatformError>>,
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
        let session = match Session::connect() {
            Ok(session) => session,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(()));
        if let Err(error) = session.run(shared, commands).await {
            tracing::warn!(%error, "the X11 clipboard connection ended");
        }
    });
}

struct Session {
    connection: RustConnection,
    readable: AsyncFd<OwnedFd>,
    window: Window,
    atoms: Atoms,
    /// Wye's text while it owns the clipboard.
    owned: Option<String>,
}

impl Session {
    fn connect() -> Result<Self, PlatformError> {
        let (connection, screen) = RustConnection::connect(None)
            .map_err(|error| PlatformError::Unavailable(format!("no X display: {error}")))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| PlatformError::Failed("the X server named no screen".to_owned()))?;
        connection
            .xfixes_query_version(5, 0)
            .map_err(x11)?
            .reply()
            .map_err(|error| PlatformError::Unavailable(format!("no XFIXES: {error}")))?;
        let window = connection.generate_id().map_err(x11)?;
        connection
            .create_window(
                0,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &CreateWindowAux::new(),
            )
            .map_err(x11)?;
        let atoms = Atoms {
            clipboard: intern(&connection, "CLIPBOARD")?,
            targets: intern(&connection, "TARGETS")?,
            utf8: intern(&connection, "UTF8_STRING")?,
            text: intern(&connection, "TEXT")?,
            property: intern(&connection, "WYE_CLIPBOARD")?,
            password_hint: intern(&connection, PASSWORD_HINT)?,
        };
        connection
            .xfixes_select_selection_input(
                window,
                atoms.clipboard,
                SelectionEventMask::SET_SELECTION_OWNER
                    | SelectionEventMask::SELECTION_WINDOW_DESTROY
                    | SelectionEventMask::SELECTION_CLIENT_CLOSE,
            )
            .map_err(x11)?;
        // Read what is there now, as if it had just been copied.
        connection
            .convert_selection(
                window,
                atoms.clipboard,
                atoms.targets,
                atoms.property,
                CURRENT_TIME,
            )
            .map_err(x11)?;
        connection.flush().map_err(x11)?;
        let fd = connection
            .stream()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|error| PlatformError::Failed(error.to_string()))?;
        let readable = AsyncFd::with_interest(fd, Interest::READABLE)
            .map_err(|error| PlatformError::Failed(error.to_string()))?;
        Ok(Self {
            connection,
            readable,
            window,
            atoms,
            owned: None,
        })
    }

    async fn run(
        mut self,
        shared: &Arc<Shared>,
        mut commands: mpsc::UnboundedReceiver<(String, oneshot::Sender<()>)>,
    ) -> Result<(), PlatformError> {
        loop {
            while let Some(event) = self.connection.poll_for_event().map_err(x11)? {
                self.handle(shared, &event)?;
            }
            self.connection.flush().map_err(x11)?;
            tokio::select! {
                ready = self.readable.readable() => {
                    ready.map_err(|error| PlatformError::Failed(error.to_string()))?.clear_ready();
                }
                command = commands.recv() => {
                    let Some((text, done)) = command else { return Ok(()) };
                    self.take_ownership(shared, text)?;
                    let _ = done.send(());
                }
            }
        }
    }

    fn handle(&mut self, shared: &Shared, event: &Event) -> Result<(), PlatformError> {
        match event {
            Event::XfixesSelectionNotify(notify) if notify.selection == self.atoms.clipboard => {
                if notify.owner != self.window {
                    self.owned = None;
                    self.request(self.atoms.targets)?;
                }
            }
            Event::SelectionNotify(notify) if notify.requestor == self.window => {
                self.answer(shared, notify)?;
            }
            Event::SelectionRequest(request) => self.serve(request)?,
            Event::SelectionClear(clear) if clear.selection == self.atoms.clipboard => {
                self.owned = None;
            }
            _ => {}
        }
        Ok(())
    }

    fn request(&self, target: Atom) -> Result<(), PlatformError> {
        self.connection
            .convert_selection(
                self.window,
                self.atoms.clipboard,
                target,
                self.atoms.property,
                CURRENT_TIME,
            )
            .map_err(x11)?;
        Ok(())
    }

    /// The owner answered a `ConvertSelection`: the targets, then the text.
    fn answer(&self, shared: &Shared, notify: &SelectionNotifyEvent) -> Result<(), PlatformError> {
        if notify.property == u32::from(AtomEnum::NONE) {
            set(shared, None, false);
            return Ok(());
        }
        let reply = self
            .connection
            .get_property(
                true,
                self.window,
                self.atoms.property,
                AtomEnum::ANY,
                0,
                MAX_TEXT_UNITS,
            )
            .map_err(x11)?
            .reply()
            .map_err(x11)?;
        if notify.target == self.atoms.targets {
            let targets: Vec<Atom> = reply.value32().map(Iterator::collect).unwrap_or_default();
            let blocked = targets.contains(&self.atoms.password_hint) || self.has_image(&targets);
            if blocked || !targets.contains(&self.atoms.utf8) {
                set(shared, None, false);
            } else {
                self.request(self.atoms.utf8)?;
            }
        } else if notify.target == self.atoms.utf8 && reply.bytes_after == 0 {
            let text = String::from_utf8(reply.value).ok();
            set(shared, text.as_deref(), true);
        }
        Ok(())
    }

    fn has_image(&self, targets: &[Atom]) -> bool {
        targets.iter().any(|atom| {
            self.connection
                .get_atom_name(*atom)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|name| name.name.starts_with(b"image/"))
        })
    }

    fn take_ownership(&mut self, shared: &Shared, text: String) -> Result<(), PlatformError> {
        self.connection
            .set_selection_owner(self.window, self.atoms.clipboard, CURRENT_TIME)
            .map_err(x11)?;
        set(shared, Some(&text), false);
        self.owned = Some(text);
        Ok(())
    }

    /// Another client asked for Wye's text.
    fn serve(&self, request: &SelectionRequestEvent) -> Result<(), PlatformError> {
        let property = match (&self.owned, request.target) {
            (Some(_), target) if target == self.atoms.targets => {
                let offered = [self.atoms.targets, self.atoms.utf8, self.atoms.text];
                self.connection
                    .change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        request.property,
                        AtomEnum::ATOM,
                        &offered,
                    )
                    .map_err(x11)?;
                request.property
            }
            (Some(text), target) if target == self.atoms.utf8 || target == self.atoms.text => {
                self.connection
                    .change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        request.property,
                        self.atoms.utf8,
                        text.as_bytes(),
                    )
                    .map_err(x11)?;
                request.property
            }
            _ => u32::from(AtomEnum::NONE),
        };
        let reply = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property,
        };
        self.connection
            .send_event(false, request.requestor, EventMask::NO_EVENT, reply)
            .map_err(x11)?;
        Ok(())
    }
}

/// Publish what the clipboard holds; announce other clients' text.
fn set(shared: &Shared, text: Option<&str>, foreign: bool) {
    let text = text.and_then(single_line_text);
    shared
        .current
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone_from(&text);
    if foreign && let Some(text) = text {
        let _ = shared.changes.send(text);
    }
}

fn intern(connection: &RustConnection, name: &str) -> Result<Atom, PlatformError> {
    Ok(connection
        .intern_atom(false, name.as_bytes())
        .map_err(x11)?
        .reply()
        .map_err(x11)?
        .atom)
}

fn x11(error: impl std::fmt::Display) -> PlatformError {
    PlatformError::Failed(format!("X11 clipboard: {error}"))
}
