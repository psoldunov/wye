//! `dev.soldunov.wye1`, the public interface.

use std::collections::HashMap;

use zbus::proxy;
use zbus::zvariant::Value;

use crate::Error;

/// The service's public interface (`docs/dbus-api.md`).
#[proxy(
    interface = "dev.soldunov.wye1",
    default_service = "dev.soldunov.wye",
    default_path = "/dev/soldunov/wye"
)]
pub trait Wye1 {
    /// Package version.
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;

    /// JSON [`crate::tray::TrayMenu`].
    #[zbus(property)]
    fn tray(&self) -> zbus::Result<String>;

    /// JSON [`crate::status::Status`].
    #[zbus(property)]
    fn status(&self) -> zbus::Result<String>;

    /// Bumps on every applied configuration change or reload.
    #[zbus(property)]
    fn config_revision(&self) -> zbus::Result<u64>;

    /// Bumps on every history change.
    #[zbus(property)]
    fn history_revision(&self) -> zbus::Result<u64>;

    /// Bumps when apps or profiles change (DISC-02).
    #[zbus(property)]
    fn inventory_revision(&self) -> zbus::Result<u64>;

    /// Route one link (IN-01, IN-05, IN-07). Keys: [`crate::context`].
    fn open_link(&self, url: &str, context: HashMap<&str, Value<'_>>) -> Result<(), Error>;

    /// Route the URL on the clipboard (IN-02 to IN-04).
    fn open_clipboard(&self, alternative: bool) -> Result<(), Error>;

    /// Whether the clipboard holds a URL (TRAY-10).
    fn clipboard_has_url(&self) -> Result<bool, Error>;

    /// JSON [`crate::trace::LinkTrace`] (IN-08).
    fn test_link(&self, url: &str, context: HashMap<&str, Value<'_>>) -> Result<String, Error>;

    /// Show the picker with a sample link (IN-06, PKS-06).
    fn preview_picker(&self) -> Result<(), Error>;

    /// The picker's choice (PIPE-13). Options: [`crate::context`].
    fn picker_chose(
        &self,
        request_id: &str,
        target: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> Result<(), Error>;

    /// The picker closed without a choice (PICK-23).
    fn picker_cancelled(&self, request_id: &str) -> Result<(), Error>;

    /// A [`crate::actions::PickerAction`] on a pending request.
    fn picker_action(&self, request_id: &str, action: &str) -> Result<(), Error>;

    /// The configuration as JSON, and its revision.
    fn get_config(&self) -> Result<(String, u64), Error>;

    /// Apply an RFC 7386 merge patch (SET-06); returns the new revision.
    fn update_config(&self, merge_patch: &str, base_revision: u64) -> Result<u64, Error>;

    /// Set the primary browser (TRAY-11). `target` is a
    /// [`crate::TargetSpec`] as JSON.
    fn set_primary(&self, target: &str) -> Result<(), Error>;

    /// Default values of one configuration section as JSON (KEY-04).
    fn get_defaults(&self, section: &str) -> Result<String, Error>;

    /// JSON [`crate::targets::TargetInventory`].
    fn get_targets(&self) -> Result<String, Error>;

    /// JSON [`crate::apps::AppList`].
    fn get_apps(&self, all: bool) -> Result<String, Error>;

    /// JSON [`crate::services::ServiceList`].
    fn get_services(&self) -> Result<String, Error>;

    /// JSON [`crate::expansion::ExpansionCatalogue`].
    fn get_expansion_catalogue(&self) -> Result<String, Error>;

    /// Rediscover apps and profiles (TRAY-15, BRW-06).
    fn rescan(&self) -> Result<(), Error>;

    /// Make Wye the default browser (DEF-02).
    fn make_default(&self) -> Result<(), Error>;

    /// Restore the previous default browser (DEF-05).
    fn stop_being_default(&self) -> Result<(), Error>;

    /// Keep another default browser and stop asking (ONB-11).
    fn keep_current_default(&self) -> Result<(), Error>;

    /// Rules and their scripts as TOML (RUL-02).
    fn export_rules(&self) -> Result<String, Error>;

    /// Append rules from TOML; returns how many (RUL-02).
    fn import_rules(&self, text: &str) -> Result<u32, Error>;

    /// Source of a script ([`crate::actions::ScriptScope`]).
    fn get_script(&self, scope: &str) -> Result<String, Error>;

    /// Save a script; `ScriptSyntax` when it does not compile (SCR-07).
    fn set_script(&self, scope: &str, source: &str) -> Result<(), Error>;

    /// Whether a script has a file with more than whitespace (SCR-09).
    fn script_exists(&self, scope: &str) -> Result<bool, Error>;

    /// JSON [`crate::scripts::ScriptRun`] (SCR-04).
    fn run_script(
        &self,
        source: &str,
        url: &str,
        context: HashMap<&str, Value<'_>>,
    ) -> Result<String, Error>;

    /// JSON [`crate::history::History`].
    fn get_history(&self) -> Result<String, Error>;

    /// Forget every entry (DLG-HIS-01, ADV-09).
    fn clear_history(&self) -> Result<(), Error>;

    /// Forget one entry (DLG-HIS-03).
    fn delete_history_entry(&self, id: u64) -> Result<(), Error>;

    /// Open an entry again ([`crate::actions::Reopen`]).
    fn reopen_history_entry(&self, id: u64, how: &str) -> Result<(), Error>;

    /// JSON [`crate::shortcuts::Shortcuts`] (KEY-40, KEY-41).
    fn get_shortcuts(&self) -> Result<String, Error>;

    /// Bind an action; an empty binding clears it.
    fn set_shortcut(&self, action: &str, binding: &str) -> Result<(), Error>;

    /// Open the shortcut mechanism's own dialog (KEY-40).
    fn configure_shortcuts(&self) -> Result<(), Error>;

    /// Apply a merge patch to [`crate::status::UiState`].
    fn update_ui_state(&self, merge_patch: &str) -> Result<(), Error>;

    /// Open a [`crate::actions::Window`] in the UI host.
    fn show_window(&self, window: &str, argument: &str) -> Result<(), Error>;

    /// Open or close the tray menu popup (TRAY-08).
    fn toggle_menu(&self) -> Result<(), Error>;

    /// Announce an external tray host ([`crate::actions::TrayHost`]); the
    /// service hides its own tray icon while the caller is connected. Wye
    /// ships no such host: the call is kept for older applets and
    /// third-party hosts.
    fn register_tray(&self, kind: &str) -> Result<(), Error>;

    /// The caller no longer shows the tray; the service's own tray icon comes
    /// back at once.
    fn unregister_tray(&self) -> Result<(), Error>;

    /// Carry out the tray item `id` of the `Tray` model (01-tray-menu.md).
    fn activate_tray_item(&self, id: &str) -> Result<(), Error>;

    /// Troubleshooting text for the About window (DLG-ABT-02).
    fn get_troubleshooting(&self) -> Result<String, Error>;

    /// Quit the service (TRAY-17).
    fn quit(&self) -> Result<(), Error>;

    /// A tray host that can open its own menu may do so now.
    #[zbus(signal)]
    fn menu_requested(&self) -> zbus::Result<()>;

    /// A script file changed on disk (SCR-08).
    #[zbus(signal)]
    fn script_file_changed(&self, scope: &str) -> zbus::Result<()>;
}
