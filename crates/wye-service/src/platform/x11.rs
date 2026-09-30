//! X11 session probes: pointer (`QueryPointer` and the `RandR` monitor under
//! it, PICK-02), held modifiers (its mask, KEY-06) and the active window
//! (`_NET_ACTIVE_WINDOW`, then `_NET_WM_PID`, `_KDE_NET_WM_DESKTOP_FILE` or
//! `_GTK_APPLICATION_ID`, and `WM_CLASS`; source-app step 4).
//!
//! x11rb's pure-Rust connection is blocking, so each probe connects on a
//! blocking thread and gives up after [`TIMEOUT`].

pub mod parse;

use std::time::Duration;

use async_trait::async_trait;
use wye_api::context::Modifier;
use wye_api::picker::Placement;
use x11rb::connection::Connection as _;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

use self::parse::Monitor;
use super::{FocusSource, FocusedApp, PlatformError, PointerSource};

/// Mechanism name in `Status.capabilities`.
pub const MECHANISM: &str = "x11";

/// How long one probe may take.
pub const TIMEOUT: Duration = Duration::from_millis(150);

/// Longest property value read, in 32-bit units.
const PROPERTY_LENGTH: u32 = 1024;

/// X11 pointer and focus.
#[derive(Debug, Clone, Copy, Default)]
pub struct X11Probe;

impl X11Probe {
    /// The start-up self-check: the X server is reachable.
    ///
    /// # Errors
    ///
    /// When it is not.
    pub async fn check() -> Result<Self, PlatformError> {
        with_connection(|_, _| Ok(())).await.map(|()| Self)
    }
}

#[async_trait]
impl PointerSource for X11Probe {
    async fn pointer(&self) -> Option<Placement> {
        with_connection(pointer)
            .await
            .inspect_err(|error| tracing::info!(%error, "pointer unknown"))
            .ok()
            .flatten()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

#[async_trait]
impl FocusSource for X11Probe {
    async fn focused(&self) -> Option<FocusedApp> {
        with_connection(active_window)
            .await
            .inspect_err(|error| tracing::info!(%error, "focused app unknown"))
            .ok()
            .flatten()
    }

    fn mechanism(&self) -> Option<&'static str> {
        Some(MECHANISM)
    }
}

/// The modifiers held now, from the pointer's mask.
///
/// # Errors
///
/// When the X server cannot be asked in time.
pub async fn held_modifiers() -> Result<Vec<Modifier>, PlatformError> {
    with_connection(|connection, root| {
        let reply = connection
            .query_pointer(root)
            .map_err(failed)?
            .reply()
            .map_err(failed)?;
        Ok(parse::modifiers(u16::from(reply.mask)))
    })
    .await
}

/// Connect on a blocking thread and run `work` with the default screen's
/// root window.
async fn with_connection<T: Send + 'static>(
    work: fn(&RustConnection, Window) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    let task = tokio::task::spawn_blocking(move || {
        let (connection, screen) = RustConnection::connect(None)
            .map_err(|error| PlatformError::Unavailable(format!("no X server: {error}")))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .ok_or_else(|| PlatformError::Failed(format!("the X server has no screen {screen}")))?
            .root;
        work(&connection, root)
    });
    match tokio::time::timeout(TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(PlatformError::Failed(format!(
            "the X11 probe stopped: {error}"
        ))),
        Err(_) => Err(PlatformError::Timeout(TIMEOUT)),
    }
}

fn pointer(connection: &RustConnection, root: Window) -> Result<Option<Placement>, PlatformError> {
    let reply = connection
        .query_pointer(root)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    let monitors = monitors(connection, root)?;
    Ok(parse::placement(
        &monitors,
        i32::from(reply.root_x),
        i32::from(reply.root_y),
    ))
}

fn monitors(connection: &RustConnection, root: Window) -> Result<Vec<Monitor>, PlatformError> {
    let reply = connection
        .randr_get_monitors(root, true)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    reply
        .monitors
        .iter()
        .map(|monitor| {
            Ok(Monitor {
                name: atom_name(connection, monitor.name)?,
                x: i32::from(monitor.x),
                y: i32::from(monitor.y),
                width: i32::from(monitor.width),
                height: i32::from(monitor.height),
            })
        })
        .collect()
}

fn active_window(
    connection: &RustConnection,
    root: Window,
) -> Result<Option<FocusedApp>, PlatformError> {
    let active = atom(connection, "_NET_ACTIVE_WINDOW")?;
    let window = property(connection, root, active, AtomEnum::WINDOW.into())?
        .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
        .filter(|window| *window != 0);
    let Some(window) = window else {
        return Ok(None);
    };
    let pid = property(
        connection,
        window,
        atom(connection, "_NET_WM_PID")?,
        AtomEnum::CARDINAL.into(),
    )?
    .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
    .filter(|pid| *pid != 0);
    let desktop_id = text_property(connection, window, "_KDE_NET_WM_DESKTOP_FILE")?
        .or(text_property(connection, window, "_GTK_APPLICATION_ID")?);
    let resource_class = property(
        connection,
        window,
        AtomEnum::WM_CLASS.into(),
        AtomEnum::STRING.into(),
    )?
    .and_then(|reply| parse::wm_class(&reply.value));
    let app = FocusedApp {
        pid,
        desktop_id,
        resource_class,
    };
    Ok((app != FocusedApp::default()).then_some(app))
}

fn text_property(
    connection: &RustConnection,
    window: Window,
    name: &str,
) -> Result<Option<String>, PlatformError> {
    let name = atom(connection, name)?;
    Ok(property(connection, window, name, AtomEnum::ANY.into())?
        .and_then(|reply| parse::text(&reply.value)))
}

fn property(
    connection: &RustConnection,
    window: Window,
    name: Atom,
    kind: Atom,
) -> Result<Option<x11rb::protocol::xproto::GetPropertyReply>, PlatformError> {
    let reply = connection
        .get_property(false, window, name, kind, 0, PROPERTY_LENGTH)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    Ok((reply.type_ != u32::from(AtomEnum::NONE)).then_some(reply))
}

fn atom(connection: &RustConnection, name: &str) -> Result<Atom, PlatformError> {
    Ok(connection
        .intern_atom(false, name.as_bytes())
        .map_err(failed)?
        .reply()
        .map_err(failed)?
        .atom)
}

fn atom_name(connection: &RustConnection, atom: Atom) -> Result<String, PlatformError> {
    let reply = connection
        .get_atom_name(atom)
        .map_err(failed)?
        .reply()
        .map_err(failed)?;
    Ok(String::from_utf8_lossy(&reply.name).into_owned())
}

fn failed(error: impl std::fmt::Display) -> PlatformError {
    PlatformError::Failed(format!("X11: {error}"))
}
