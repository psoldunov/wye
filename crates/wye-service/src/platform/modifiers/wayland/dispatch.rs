//! What the probe's Wayland objects report, collected into [`Probe`].

use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt as _;

use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_region, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use super::super::xkb::Mask;

/// Largest keymap accepted; real ones are around 60 KiB.
const MAX_KEYMAP: u32 = 4 * 1024 * 1024;

/// One global the compositor announced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

/// Everything the probe learned so far.
#[derive(Debug, Default)]
pub struct Probe {
    /// Globals from the registry.
    pub globals: Vec<Global>,
    /// The registry's `sync` came back: `globals` is complete.
    pub synced: bool,
    /// Serial of the layer surface's configure, to acknowledge.
    pub configure: Option<u32>,
    /// The compositor closed the layer surface.
    pub closed: bool,
    /// Keymap text (`xkb_v1`).
    pub keymap: Option<String>,
    /// The surface has keyboard focus.
    pub entered: bool,
    /// Modifier state sent while focused.
    pub mask: Option<Mask>,
}

impl Probe {
    /// The modifiers and the keymap to read them with, once both came.
    pub fn answer(&self) -> Option<(&str, Mask)> {
        Some((self.keymap.as_deref()?, self.mask?))
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            state.globals.push(Global {
                name,
                interface,
                version,
            });
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Probe {
    fn event(
        state: &mut Self,
        _callback: &wl_callback::WlCallback,
        event: wl_callback::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.synced = true;
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Probe {
    fn event(
        state: &mut Self,
        _surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                state.configure = Some(serial);
            }
            zwlr_layer_surface_v1::Event::Closed => state.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Probe {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    state.keymap = read_keymap(fd, size)
                        .inspect_err(|error| tracing::info!(%error, "cannot read the keymap"))
                        .ok();
                }
            }
            wl_keyboard::Event::Enter { .. } => state.entered = true,
            wl_keyboard::Event::Leave { .. } => state.entered = false,
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } if state.entered => {
                state.mask = Some(Mask {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                });
            }
            _ => {}
        }
    }
}

delegate_noop!(Probe: wl_compositor::WlCompositor);
delegate_noop!(Probe: wl_shm_pool::WlShmPool);
delegate_noop!(Probe: wl_region::WlRegion);
delegate_noop!(Probe: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Probe: ignore wl_shm::WlShm);
delegate_noop!(Probe: ignore wl_seat::WlSeat);
delegate_noop!(Probe: ignore wl_surface::WlSurface);
delegate_noop!(Probe: ignore wl_buffer::WlBuffer);

/// The keymap text in `fd`, read at offset 0 without moving the file
/// offset the compositor may share with other clients.
fn read_keymap(fd: OwnedFd, size: u32) -> std::io::Result<String> {
    if size > MAX_KEYMAP {
        return Err(std::io::Error::other(format!("a {size}-byte keymap")));
    }
    let file = File::from(fd);
    let mut bytes = vec![0; usize::try_from(size).map_err(std::io::Error::other)?];
    file.read_exact_at(&mut bytes, 0)?;
    // The text ends in a NUL.
    let text = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    String::from_utf8(text.to_vec()).map_err(std::io::Error::other)
}
