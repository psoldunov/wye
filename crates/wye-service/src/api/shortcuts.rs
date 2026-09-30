//! Global shortcuts and the tray-menu popup: `GetShortcuts`, `SetShortcut`,
//! `ConfigureShortcuts`, `ToggleMenu` (KEY-40, KEY-41, ADV-05 to ADV-07,
//! TRAY-08).
//!
//! `ToggleMenu` lives here because the toggle-menu shortcut is its main
//! caller; it shows the UI host's popup (decision 4). A shortcut press
//! arrives from the platform's [`ShortcutProvider`] and runs the same code
//! as `ToggleMenu` and `OpenClipboard`.
//!
//! [`ShortcutProvider`]: crate::platform::ShortcutProvider

mod menu;

use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;
use wye_api::Error;
use wye_api::json;
use wye_api::shortcuts::{
    CLIPBOARD_ALTERNATIVE, CLIPBOARD_PRIMARY, ShortcutBinding, ShortcutMechanism, Shortcuts,
    TOGGLE_MENU,
};
use wye_core::keybinding::KeyBinding;

pub use self::menu::toggle_menu;
use super::{Caller, Result};
use crate::context::ServiceContext;
use crate::platform::shortcuts::{ACTIONS, ShortcutAction, action};
use crate::platform::{BoundShortcut, Platform};

/// State this topic keeps: none; the provider keeps the bindings.
#[derive(Debug, Default)]
pub struct State;

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self
    }
}

/// `dev.soldunov.wye1.GetShortcuts`: JSON [`wye_api::shortcuts::Shortcuts`],
/// one entry per action with the trigger the mechanism reports (KEY-40) and
/// the command to bind by hand (KEY-41).
pub async fn get_shortcuts(ctx: &ServiceContext) -> Result<String> {
    let provider = &ctx.platform().shortcuts;
    let bound = provider
        .bindings()
        .await
        .inspect_err(|error| tracing::debug!(%error, "cannot list the global shortcuts"))
        .unwrap_or_default();
    json::encode(&Shortcuts {
        mechanism: mechanism(provider.mechanism()),
        bindings: ACTIONS
            .iter()
            .map(|action| binding(action, &bound))
            .collect(),
    })
}

/// `dev.soldunov.wye1.SetShortcut`: save `binding` as the preferred trigger
/// of `action` (ADV-05 to ADV-07) and bind it; an empty binding clears the
/// preference. On the portal the desktop has the last word: a trigger the
/// user chose in its settings stays (KEY-40).
///
/// # Errors
///
/// `InvalidArgs` for an unknown action or a binding Wye cannot read,
/// `Unavailable` without a mechanism (KEY-41), the configuration's errors
/// when it cannot be saved.
pub async fn set_shortcut(ctx: &ServiceContext, action_id: &str, binding: &str) -> Result<()> {
    let action = action(action_id)
        .ok_or_else(|| Error::invalid_args(format!("unknown shortcut action `{action_id}`")))?;
    let stored = stored_binding(binding)?;
    let provider = &ctx.platform().shortcuts;
    if provider.mechanism().is_none() {
        return Err(Error::Unavailable(format!(
            "this session has no global shortcuts; bind `{}` in its settings",
            action.command
        )));
    }
    let key = config_key(action);
    let value = stored
        .as_deref()
        .map_or(serde_json::Value::Null, serde_json::Value::from);
    let patch = serde_json::json!({ "shortcuts": { key: value } });
    super::config::apply_patch(ctx, &patch, 0).await?;
    provider
        .bind(action.id, stored.as_deref().unwrap_or_default())
        .await
        .map_err(Error::from)
}

/// `dev.soldunov.wye1.ConfigureShortcuts` (KEY-40 "Change…"): the
/// mechanism's own dialog.
///
/// # Errors
///
/// `Unavailable` when the mechanism has none.
pub async fn configure_shortcuts(ctx: &ServiceContext) -> Result<()> {
    ctx.platform()
        .shortcuts
        .configure()
        .await
        .map_err(Error::from)
}

/// The task that runs each shortcut press.
pub(crate) fn spawn_tasks(ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    vec![tokio::spawn(run_presses(ctx.clone()))]
}

async fn run_presses(ctx: ServiceContext) {
    let mut presses = ctx.platform().shortcuts.activations();
    loop {
        match presses.recv().await {
            Ok(id) => {
                tokio::spawn(press(ctx.clone(), id));
            }
            Err(RecvError::Lagged(missed)) => {
                tracing::warn!(missed, "global shortcut presses were dropped");
            }
            Err(RecvError::Closed) => return,
        }
    }
}

/// Run the action of one press (TRAY-08, IN-03, IN-04).
async fn press(ctx: ServiceContext, id: String) {
    let result = match id.as_str() {
        TOGGLE_MENU => toggle_menu(&ctx).await,
        CLIPBOARD_PRIMARY => {
            super::clipboard::open_clipboard(&ctx, &Caller::default(), false).await
        }
        CLIPBOARD_ALTERNATIVE => {
            super::clipboard::open_clipboard(&ctx, &Caller::default(), true).await
        }
        other => {
            tracing::debug!(id = other, "press of an unknown shortcut");
            return;
        }
    };
    if let Err(error) = result {
        tracing::info!(%error, id, "global shortcut did nothing");
    }
}

/// The wire name of a platform's mechanism. A mechanism other than X11
/// key grabs owns the bindings the way the portal does.
fn mechanism(name: Option<&str>) -> ShortcutMechanism {
    match name {
        None => ShortcutMechanism::None,
        Some(name) if name == ShortcutMechanism::X11.as_str() => ShortcutMechanism::X11,
        Some(_) => ShortcutMechanism::Portal,
    }
}

fn binding(action: &ShortcutAction, bound: &[BoundShortcut]) -> ShortcutBinding {
    ShortcutBinding {
        action: action.id.to_owned(),
        description: action.description.to_owned(),
        trigger: bound
            .iter()
            .find(|shortcut| shortcut.action == action.id)
            .and_then(|shortcut| shortcut.trigger.clone()),
        command: action.command.to_owned(),
    }
}

/// `binding` in the configuration's form (KEY-03), or `None` when empty.
fn stored_binding(binding: &str) -> Result<Option<String>> {
    let binding = binding.trim();
    if binding.is_empty() {
        return Ok(None);
    }
    binding
        .parse::<KeyBinding>()
        .map(|parsed| Some(parsed.stored()))
        .map_err(|error| Error::invalid_args(format!("shortcut `{binding}`: {error}")))
}

/// The `[shortcuts]` key of an action; the action IDs are the keys.
const fn config_key(action: &ShortcutAction) -> &'static str {
    action.id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanisms_map_to_their_wire_names() {
        assert_eq!(mechanism(None), ShortcutMechanism::None);
        assert_eq!(mechanism(Some("portal")), ShortcutMechanism::Portal);
        assert_eq!(mechanism(Some("x11")), ShortcutMechanism::X11);
    }

    #[test]
    fn bindings_are_stored_in_canonical_form_key_03() {
        assert_eq!(
            stored_binding(" control+ALT+W ").expect("reads"),
            Some("Ctrl+Alt+w".to_owned())
        );
        assert_eq!(stored_binding("").expect("reads"), None);
        assert!(stored_binding("Hyper+w").is_err());
    }

    #[test]
    fn an_unbound_action_still_names_its_command_key_41() {
        let entry = binding(&ACTIONS[1], &[]);
        assert_eq!(entry.action, CLIPBOARD_PRIMARY);
        assert_eq!(entry.trigger, None);
        assert_eq!(entry.command, "wye clipboard");
    }
}
