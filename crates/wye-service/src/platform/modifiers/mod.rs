//! Held-modifier detection (BRW-03, RUL-27, ADV-11, KEY-06).
//!
//! When a link arrives the service asks which modifiers are held right now:
//!
//! - Wayland ([`wayland`]): a 1×1 fully transparent layer-shell surface on
//!   the overlay layer with exclusive keyboard focus; the compositor sends
//!   the keyboard's modifier state when it gives the surface focus, and the
//!   surface is destroyed at once.
//! - X11 ([`x11`]): the modifier mask of `QueryPointer`.
//!
//! Each probe gives up after [`PROBE_TIMEOUT`]; then the modifiers are
//! unknown. The setting `advanced.held-keys = "auto" | "off"` switches
//! probing off ([`Configured`]). A probe is only worth its cost when a
//! binding depends on modifiers ([`bindings_need_modifiers`]).

pub mod wayland;
pub mod x11;
pub mod xkb;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use wye_api::context::Modifier;
pub use wye_core::config::HeldKeys;
use wye_core::{Config, EntryPoint};

use super::ModifierSource;

/// How long one probe may take before the modifiers count as unknown.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(150);

/// Modifiers are never known.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoModifiers;

#[async_trait]
impl ModifierSource for NoModifiers {
    async fn held(&self) -> Option<Vec<Modifier>> {
        None
    }

    fn mechanism(&self) -> Option<&'static str> {
        None
    }
}

/// The setting in the text of `config.toml` (`advanced.held-keys`).
///
/// Read leniently, as the configuration loader reads it: a missing key, a
/// value of the wrong kind or a file that is not TOML all mean
/// [`HeldKeys::Auto`]; the loader reports broken files.
#[must_use]
pub fn held_keys_in(text: &str) -> HeldKeys {
    Config::parse(text, &[]).map_or(HeldKeys::Auto, |loaded| loaded.config.advanced.held_keys)
}

/// The setting in the file at `path`; [`HeldKeys::Auto`] when there is no
/// file.
pub async fn load_held_keys(path: &Path) -> HeldKeys {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => held_keys_in(&text),
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(%error, path = %path.display(), "cannot read the configuration");
            }
            HeldKeys::Auto
        }
    }
}

/// Whether a link from `entry` can be routed differently depending on the
/// held modifiers, so probing them is worth it: the alternative-browser key
/// (BRW-03), a rule's held keys (RUL-27), or the bypass key on an extension
/// link with the forced picker (ADV-11).
#[must_use]
pub fn bindings_need_modifiers(config: &Config, entry: EntryPoint) -> bool {
    let alternative = !config.browsers.alternative_key.is_empty();
    let rules = config
        .rules
        .iter()
        .any(|rule| rule.enabled && !rule.held_keys.is_empty());
    let bypass = entry == EntryPoint::Extension
        && config.advanced.force_picker_from_extension
        && !config.advanced.bypass_key.is_empty();
    alternative || rules || bypass
}

/// A probe that obeys `advanced.held-keys`, read from `config` on every
/// probe so a change applies to the next link.
pub struct Configured {
    probe: Arc<dyn ModifierSource>,
    config: PathBuf,
    enabled: AtomicBool,
}

impl Configured {
    /// `probe`, switched by the setting in `config`, starting as `setting`.
    #[must_use]
    pub fn new(probe: Arc<dyn ModifierSource>, config: PathBuf, setting: HeldKeys) -> Self {
        Self {
            probe,
            config,
            enabled: AtomicBool::new(setting == HeldKeys::Auto),
        }
    }
}

impl std::fmt::Debug for Configured {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Configured")
            .field("probe", &self.probe.mechanism())
            .field("config", &self.config)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[async_trait]
impl ModifierSource for Configured {
    async fn held(&self) -> Option<Vec<Modifier>> {
        let setting = load_held_keys(&self.config).await;
        self.enabled
            .store(setting == HeldKeys::Auto, Ordering::Relaxed);
        match setting {
            HeldKeys::Auto => self.probe.held().await,
            HeldKeys::Off => None,
        }
    }

    /// The probe's mechanism; none while the setting is off, so modifier
    /// choosers say held keys are unavailable (KEY-06).
    fn mechanism(&self) -> Option<&'static str> {
        if self.enabled.load(Ordering::Relaxed) {
            self.probe.mechanism()
        } else {
            None
        }
    }
}

/// The probe this session supports, found by a start-up self-check that
/// never takes focus: the layer-shell protocol and a keyboard on Wayland,
/// a reachable X server otherwise (KEY-06). [`NoModifiers`] when neither.
pub async fn detect() -> Arc<dyn ModifierSource> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return match wayland::LayerShellProbe::check().await {
            Ok(probe) => Arc::new(probe),
            Err(error) => {
                tracing::info!(%error, "held modifiers are not available on this Wayland session");
                Arc::new(NoModifiers)
            }
        };
    }
    if std::env::var_os("DISPLAY").is_some() {
        return match x11::X11Modifiers::check().await {
            Ok(probe) => Arc::new(probe),
            Err(error) => {
                tracing::info!(%error, "held modifiers are not available on this X11 session");
                Arc::new(NoModifiers)
            }
        };
    }
    Arc::new(NoModifiers)
}

/// [`detect`], obeying `advanced.held-keys` in the file at `config`.
pub async fn detect_configured(config: &Path) -> Arc<dyn ModifierSource> {
    let setting = load_held_keys(config).await;
    Arc::new(Configured::new(
        detect().await,
        config.to_path_buf(),
        setting,
    ))
}

#[cfg(test)]
mod tests;
