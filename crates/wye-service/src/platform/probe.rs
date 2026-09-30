//! `wye debug probe`: the session probes run once in-process, as the
//! service would run them for a link (KEY-06, PICK-02, source-app step 4).
//!
//! No service is needed: this process serves its own `KWin1` object on its
//! own connection, and the `KWin` script answers to that connection's
//! unique name.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use wye_api::context::Modifier;
use wye_api::names::OBJECT_PATH;
use wye_api::picker::Placement;

use super::modifiers::{HeldKeys, load_held_keys};
use super::{FocusedApp, Platform};
use crate::bus::KWin1;
use crate::context::ServiceContext;

/// One probe's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed<T> {
    /// What answered; `None` when this session has no way to ask.
    pub mechanism: Option<&'static str>,
    /// The answer; `None` when unknown.
    pub value: Option<T>,
    /// How long it took.
    pub elapsed: Duration,
}

/// Everything the service would see for a link arriving now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionProbe {
    /// `advanced.held-keys`.
    pub held_keys: HeldKeys,
    /// Held modifiers.
    pub modifiers: Probed<Vec<Modifier>>,
    /// Pointer and the output under it.
    pub pointer: Probed<Placement>,
    /// The focused window's app.
    pub focus: Probed<FocusedApp>,
}

/// Run every probe once on the session bus, in the order a link needs them:
/// modifiers first (the user may release the key), then the pointer, then
/// the focused app.
///
/// # Errors
///
/// When the session bus is unreachable or the `KWin1` object cannot be
/// served.
pub async fn run(config: Option<&Path>) -> anyhow::Result<SessionProbe> {
    let connection = zbus::Connection::session()
        .await
        .context("cannot connect to the session bus")?;
    let platform = Platform::unavailable()
        .with_session_probes(&connection, config)
        .await;
    let ctx = ServiceContext::new(platform.clone());
    connection
        .object_server()
        .at(OBJECT_PATH, KWin1::new(ctx))
        .await
        .context("cannot serve the KWin callback")?;
    let held_keys = match config {
        Some(config) => load_held_keys(config).await,
        None => HeldKeys::default(),
    };

    let started = Instant::now();
    let modifiers = platform.modifiers.held().await;
    let modifiers = Probed {
        mechanism: platform.modifiers.mechanism(),
        value: modifiers,
        elapsed: started.elapsed(),
    };
    let started = Instant::now();
    let pointer = Probed {
        value: platform.pointer.pointer().await,
        mechanism: platform.pointer.mechanism(),
        elapsed: started.elapsed(),
    };
    let started = Instant::now();
    let focus = Probed {
        value: platform.focus.focused().await,
        mechanism: platform.focus.mechanism(),
        elapsed: started.elapsed(),
    };
    Ok(SessionProbe {
        held_keys,
        modifiers,
        pointer,
        focus,
    })
}
