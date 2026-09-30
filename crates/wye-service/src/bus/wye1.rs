//! The `dev.soldunov.wye1` interface: unpacks each call and hands it to its
//! topic in [`crate::api`]. Nothing else happens here.

use wye_api::Error;
use zbus::fdo;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

use crate::api::{self, Caller, Dict};
use crate::context::ServiceContext;

/// The service's public interface (`docs/dbus-api.md`).
#[derive(Debug, Clone)]
pub struct Wye1 {
    ctx: ServiceContext,
}

impl Wye1 {
    /// The interface over `ctx`.
    #[must_use]
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }
}

/// A property that carries JSON: an encoding failure is the service's.
fn property(json: Result<String, Error>) -> fdo::Result<String> {
    json.map_err(|error| fdo::Error::Failed(error.to_string()))
}

#[zbus::interface(name = "dev.soldunov.wye1")]
impl Wye1 {
    /// Package version.
    #[zbus(property)]
    #[allow(
        clippy::unused_self,
        reason = "a D-Bus property getter takes self even when the value is constant"
    )]
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// JSON `TrayMenu`.
    #[zbus(property)]
    async fn tray(&self) -> fdo::Result<String> {
        property(api::tray::tray_json(&self.ctx).await)
    }

    /// JSON `Status`.
    #[zbus(property)]
    async fn status(&self) -> fdo::Result<String> {
        property(api::state::status_json(&self.ctx).await)
    }

    /// Bumps on every applied configuration change or reload.
    #[zbus(property)]
    async fn config_revision(&self) -> u64 {
        api::config::revision(&self.ctx).await
    }

    /// Bumps on every history change.
    #[zbus(property)]
    async fn history_revision(&self) -> u64 {
        api::history::revision(&self.ctx).await
    }

    /// Bumps when apps or profiles change (DISC-02).
    #[zbus(property)]
    async fn inventory_revision(&self) -> u64 {
        api::inventory::revision(&self.ctx).await
    }

    async fn open_link(
        &self,
        #[zbus(header)] header: Header<'_>,
        url: &str,
        context: Dict,
    ) -> Result<(), Error> {
        api::link::open_link(&self.ctx, &Caller::from_header(&header), url, &context).await
    }

    async fn open_clipboard(
        &self,
        #[zbus(header)] header: Header<'_>,
        alternative: bool,
    ) -> Result<(), Error> {
        api::clipboard::open_clipboard(&self.ctx, &Caller::from_header(&header), alternative).await
    }

    async fn clipboard_has_url(&self) -> Result<bool, Error> {
        api::clipboard::clipboard_has_url(&self.ctx).await
    }

    async fn test_link(
        &self,
        #[zbus(header)] header: Header<'_>,
        url: &str,
        context: Dict,
    ) -> Result<String, Error> {
        api::rules::test_link(&self.ctx, &Caller::from_header(&header), url, &context).await
    }

    async fn preview_picker(&self) -> Result<(), Error> {
        api::picker::preview_picker(&self.ctx).await
    }

    async fn picker_chose(
        &self,
        #[zbus(header)] header: Header<'_>,
        request_id: &str,
        target: &str,
        options: Dict,
    ) -> Result<(), Error> {
        let caller = Caller::from_header(&header);
        api::picker::picker_chose(&self.ctx, &caller, request_id, target, &options).await
    }

    async fn picker_cancelled(
        &self,
        #[zbus(header)] header: Header<'_>,
        request_id: &str,
    ) -> Result<(), Error> {
        api::picker::picker_cancelled(&self.ctx, &Caller::from_header(&header), request_id).await
    }

    async fn picker_action(
        &self,
        #[zbus(header)] header: Header<'_>,
        request_id: &str,
        action: &str,
    ) -> Result<(), Error> {
        let caller = Caller::from_header(&header);
        api::picker::picker_action(&self.ctx, &caller, request_id, action).await
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        api::config::get_config(&self.ctx).await
    }

    async fn update_config(&self, merge_patch: &str, base_revision: u64) -> Result<u64, Error> {
        api::config::update_config(&self.ctx, merge_patch, base_revision).await
    }

    async fn set_primary(&self, target: &str) -> Result<(), Error> {
        api::config::set_primary(&self.ctx, target).await
    }

    async fn get_defaults(&self, section: &str) -> Result<String, Error> {
        api::config::get_defaults(&self.ctx, section).await
    }

    async fn get_targets(&self) -> Result<String, Error> {
        api::inventory::get_targets(&self.ctx).await
    }

    async fn get_apps(&self, all: bool) -> Result<String, Error> {
        api::inventory::get_apps(&self.ctx, all).await
    }

    async fn get_services(&self) -> Result<String, Error> {
        api::inventory::get_services(&self.ctx).await
    }

    async fn get_expansion_catalogue(&self) -> Result<String, Error> {
        api::inventory::get_expansion_catalogue(&self.ctx).await
    }

    async fn rescan(&self) -> Result<(), Error> {
        api::inventory::rescan(&self.ctx).await
    }

    async fn make_default(&self) -> Result<(), Error> {
        api::default_browser::make_default(&self.ctx).await
    }

    async fn stop_being_default(&self) -> Result<(), Error> {
        api::default_browser::stop_being_default(&self.ctx).await
    }

    async fn keep_current_default(&self) -> Result<(), Error> {
        api::default_browser::keep_current_default(&self.ctx).await
    }

    async fn export_rules(&self) -> Result<String, Error> {
        api::rules::export_rules(&self.ctx).await
    }

    async fn import_rules(&self, text: &str) -> Result<u32, Error> {
        api::rules::import_rules(&self.ctx, text).await
    }

    async fn get_script(&self, scope: &str) -> Result<String, Error> {
        api::scripts::get_script(&self.ctx, scope).await
    }

    async fn set_script(&self, scope: &str, source: &str) -> Result<(), Error> {
        api::scripts::set_script(&self.ctx, scope, source).await
    }

    async fn run_script(&self, source: &str, url: &str, context: Dict) -> Result<String, Error> {
        api::scripts::run_script(&self.ctx, source, url, &context).await
    }

    async fn get_history(&self) -> Result<String, Error> {
        api::history::get_history(&self.ctx).await
    }

    async fn clear_history(&self) -> Result<(), Error> {
        api::history::clear_history(&self.ctx).await
    }

    async fn delete_history_entry(&self, id: u64) -> Result<(), Error> {
        api::history::delete_history_entry(&self.ctx, id).await
    }

    async fn reopen_history_entry(&self, id: u64, how: &str) -> Result<(), Error> {
        api::history::reopen_history_entry(&self.ctx, id, how).await
    }

    async fn get_shortcuts(&self) -> Result<String, Error> {
        api::shortcuts::get_shortcuts(&self.ctx).await
    }

    async fn set_shortcut(&self, action: &str, binding: &str) -> Result<(), Error> {
        api::shortcuts::set_shortcut(&self.ctx, action, binding).await
    }

    async fn configure_shortcuts(&self) -> Result<(), Error> {
        api::shortcuts::configure_shortcuts(&self.ctx).await
    }

    async fn update_ui_state(&self, merge_patch: &str) -> Result<(), Error> {
        api::state::update_ui_state(&self.ctx, merge_patch).await
    }

    async fn show_window(&self, window: &str, argument: &str) -> Result<(), Error> {
        api::windows::show_window(&self.ctx, window, argument).await
    }

    async fn toggle_menu(&self) -> Result<(), Error> {
        api::shortcuts::toggle_menu(&self.ctx).await
    }

    async fn register_tray(
        &self,
        #[zbus(header)] header: Header<'_>,
        kind: &str,
    ) -> Result<(), Error> {
        api::tray::register_tray(&self.ctx, &Caller::from_header(&header), kind).await
    }

    async fn get_troubleshooting(&self) -> Result<String, Error> {
        api::troubleshoot::get_troubleshooting(&self.ctx).await
    }

    async fn quit(&self) -> Result<(), Error> {
        api::windows::quit(&self.ctx).await
    }

    /// A tray host that can open its own menu may do so now.
    #[zbus(signal)]
    pub async fn menu_requested(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// A script file changed on disk (SCR-08).
    #[zbus(signal)]
    pub async fn script_file_changed(emitter: &SignalEmitter<'_>, scope: &str) -> zbus::Result<()>;
}
