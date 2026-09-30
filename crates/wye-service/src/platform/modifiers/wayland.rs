//! Held modifiers on Wayland through a transient layer-shell surface
//! (KEY-06; risk 1 of the design).
//!
//! A client may not read the keyboard until one of its surfaces has
//! keyboard focus. The probe maps a 1×1 fully transparent surface on the
//! overlay layer with `KeyboardInteractivity::Exclusive` and an empty input
//! region; the compositor focuses it and sends `wl_keyboard.enter` followed
//! by `modifiers`, which xkbcommon reads with the keymap the compositor
//! sent. The connection is then closed, which destroys the surface and gives
//! focus back. Needs `zwlr_layer_shell_v1` (`KWin`, wlroots compositors;
//! not GNOME).

mod dispatch;

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use wayland_client::backend::WaylandError;
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use wye_api::context::Modifier;

use self::dispatch::{Global, Probe};
use super::xkb::{self, Mask};
use super::{ModifierSource, PROBE_TIMEOUT};
use crate::platform::PlatformError;

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "wayland-layer-shell";

/// The layer surface's namespace; compositors may match rules on it.
const NAMESPACE: &str = "wye-probe";

/// Highest versions the probe uses.
const COMPOSITOR_VERSION: u32 = 4;
const SHM_VERSION: u32 = 1;
const SEAT_VERSION: u32 = 5;
const LAYER_SHELL_VERSION: u32 = 4;

/// One ARGB pixel.
const PIXEL_BYTES: i32 = 4;

/// Held modifiers from a transient layer surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerShellProbe;

impl LayerShellProbe {
    /// The start-up self-check: the compositor offers everything the probe
    /// needs. Maps nothing and takes no focus.
    ///
    /// # Errors
    ///
    /// When there is no Wayland display, or a global is missing.
    pub async fn check() -> Result<Self, PlatformError> {
        tokio::time::timeout(PROBE_TIMEOUT, async {
            let mut session = Session::connect()?;
            session.globals().await?;
            Ok(Self)
        })
        .await
        .unwrap_or(Err(PlatformError::Timeout(PROBE_TIMEOUT)))
    }

    /// Probe once.
    ///
    /// # Errors
    ///
    /// When the compositor refused, or did not focus the surface in
    /// [`PROBE_TIMEOUT`].
    pub async fn probe(&self) -> Result<Vec<Modifier>, PlatformError> {
        let (keymap, mask) = tokio::time::timeout(PROBE_TIMEOUT, async {
            let mut session = Session::connect()?;
            let globals = session.globals().await?;
            session.focus_and_read(&globals).await
        })
        .await
        .unwrap_or(Err(PlatformError::Timeout(PROBE_TIMEOUT)))?;
        xkb::held(&keymap, mask)
    }
}

#[async_trait]
impl ModifierSource for LayerShellProbe {
    async fn held(&self) -> Option<Vec<Modifier>> {
        self.probe()
            .await
            .inspect_err(|error| tracing::info!(%error, "held modifiers unknown"))
            .ok()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// The globals the probe binds.
struct Globals {
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    seat: wl_seat::WlSeat,
    layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
}

/// One connection to the compositor, driven on the runtime.
struct Session {
    connection: Connection,
    queue: EventQueue<Probe>,
    handle: QueueHandle<Probe>,
    readable: AsyncFd<OwnedFd>,
    state: Probe,
}

impl Session {
    fn connect() -> Result<Self, PlatformError> {
        let connection = Connection::connect_to_env()
            .map_err(|error| PlatformError::Unavailable(format!("no Wayland display: {error}")))?;
        let fd = connection
            .backend()
            .poll_fd()
            .try_clone_to_owned()
            .map_err(|error| failed("the Wayland socket", &error))?;
        let readable = AsyncFd::with_interest(fd, Interest::READABLE)
            .map_err(|error| failed("the Wayland socket", &error))?;
        let queue = connection.new_event_queue();
        let handle = queue.handle();
        Ok(Self {
            connection,
            queue,
            handle,
            readable,
            state: Probe::default(),
        })
    }

    /// Read the registry and bind what the probe needs.
    async fn globals(&mut self) -> Result<Globals, PlatformError> {
        let display = self.connection.display();
        let registry = display.get_registry(&self.handle, ());
        display.sync(&self.handle, ());
        self.run_until(|state| state.synced).await?;
        let announced = &self.state.globals;
        Ok(Globals {
            compositor: bind(&registry, &self.handle, announced, COMPOSITOR_VERSION)?,
            shm: bind(&registry, &self.handle, announced, SHM_VERSION)?,
            seat: bind(&registry, &self.handle, announced, SEAT_VERSION)?,
            layer_shell: bind(&registry, &self.handle, announced, LAYER_SHELL_VERSION)?,
        })
    }

    /// Map the surface, wait for focus and the modifiers, unmap it.
    async fn focus_and_read(&mut self, globals: &Globals) -> Result<(String, Mask), PlatformError> {
        let handle = &self.handle;
        let keyboard = globals.seat.get_keyboard(handle, ());
        let surface = globals.compositor.create_surface(handle, ());
        let nothing = globals.compositor.create_region(handle, ());
        surface.set_input_region(Some(&nothing));
        nothing.destroy();
        let layer = globals.layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            NAMESPACE.to_owned(),
            handle,
            (),
        );
        layer.set_size(1, 1);
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
        surface.commit();

        self.run_until(|state| state.configure.is_some()).await?;
        if let Some(serial) = self.state.configure {
            layer.ack_configure(serial);
        }
        let buffer = transparent_pixel(&globals.shm, &self.handle)?;
        surface.attach(Some(&buffer), 0, 0);
        surface.damage(0, 0, 1, 1);
        surface.commit();

        self.run_until(|state| state.answer().is_some()).await?;
        let answer = self
            .state
            .answer()
            .map(|(keymap, mask)| (keymap.to_owned(), mask));

        layer.destroy();
        surface.destroy();
        buffer.destroy();
        if keyboard.version() >= 3 {
            keyboard.release();
        }
        // Closing the connection cleans up too; flushing only speeds it.
        let _ = self.connection.flush();
        answer.ok_or_else(|| PlatformError::Failed("the compositor closed the probe".to_owned()))
    }

    /// Dispatch events until `done` holds or the surface is closed.
    async fn run_until(&mut self, done: impl Fn(&Probe) -> bool) -> Result<(), PlatformError> {
        loop {
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(|error| failed("Wayland events", &error))?;
            if done(&self.state) || self.state.closed {
                return Ok(());
            }
            self.queue
                .flush()
                .map_err(|error| failed("the Wayland socket", &error))?;
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };
            let mut ready = self
                .readable
                .readable()
                .await
                .map_err(|error| failed("the Wayland socket", &error))?;
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    ready.clear_ready();
                }
                Err(error) => return Err(failed("the Wayland socket", &error)),
            }
        }
    }
}

/// Bind the announced global of interface `I`.
fn bind<I>(
    registry: &wl_registry::WlRegistry,
    handle: &QueueHandle<Probe>,
    announced: &[Global],
    highest: u32,
) -> Result<I, PlatformError>
where
    I: Proxy + 'static,
    Probe: Dispatch<I, ()>,
{
    let name = I::interface().name;
    let global = announced
        .iter()
        .find(|global| global.interface == name)
        .ok_or_else(|| PlatformError::Unavailable(format!("the compositor has no {name}")))?;
    Ok(registry.bind(global.name, global.version.min(highest), handle, ()))
}

/// A 1×1 fully transparent buffer, from a file unlinked at once.
fn transparent_pixel(
    shm: &wl_shm::WlShm,
    handle: &QueueHandle<Probe>,
) -> Result<wl_buffer::WlBuffer, PlatformError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let path = dir.join(format!(
        "wye-probe-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| failed("the pixel buffer", &error))?;
    let written = file.write_all(&[0; 4]);
    if let Err(error) = std::fs::remove_file(&path) {
        tracing::warn!(%error, path = %path.display(), "cannot remove the pixel buffer file");
    }
    written.map_err(|error| failed("the pixel buffer", &error))?;
    let pool = shm.create_pool(file.as_fd(), PIXEL_BYTES, handle, ());
    let buffer = pool.create_buffer(0, 1, 1, PIXEL_BYTES, wl_shm::Format::Argb8888, handle, ());
    pool.destroy();
    Ok(buffer)
}

fn failed(what: &str, error: &dyn std::fmt::Display) -> PlatformError {
    PlatformError::Failed(format!("{what}: {error}"))
}
