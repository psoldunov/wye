//! What the pages read beyond plain values: help texts (BLK-08),
//! dismissed callouts (BLK-09), the target menus (TGT-01, TGT-02).

use serde_json::Value;

use super::SettingsStore;
use crate::settings::help;
use crate::settings::menu::{self, Request, Row, Surface};
use crate::settings::snapshot::Snapshot;

impl SettingsStore {
    /// The help popover text for `id` (BLK-08, 19-help-texts.md); empty for
    /// an unknown ID.
    #[must_use]
    pub fn help_text(&self, id: &str) -> String {
        self.with_snapshot(|snapshot| {
            let key = snapshot.typed_config().browsers.alternative_key;
            let shown = if key.is_empty() {
                "the alternative-browser key".to_owned()
            } else {
                key.to_string()
            };
            let context = help::Context {
                alternative_key: &shown,
                held_keys_available: snapshot.held_keys_available(),
            };
            help::text(id, &context).unwrap_or_default()
        })
    }

    /// Whether the user dismissed callout `id` (BLK-09).
    #[must_use]
    pub fn callout_dismissed(&self, id: &str) -> bool {
        self.with_snapshot(|snapshot| snapshot.callout_dismissed(id))
    }

    /// The target menu's rows for `surface`, with `current` checked (TGT-02);
    /// `service` is the mapped web service on the Apps page, else empty.
    #[must_use]
    pub fn target_menu(&self, surface: Surface, current: &Value, service: &str) -> Vec<Row> {
        self.with_menu_request(surface, current, service, |snapshot, request| {
            menu::build(&snapshot.targets, request)
        })
    }

    /// The row the closed target popup shows for `current` (TGT-01).
    #[must_use]
    pub fn target_label(&self, surface: Surface, current: &Value, service: &str) -> Row {
        self.with_menu_request(surface, current, service, |snapshot, request| {
            menu::describe(&snapshot.targets, request)
        })
    }

    fn with_menu_request<T>(
        &self,
        surface: Surface,
        current: &Value,
        service: &str,
        build: impl FnOnce(&Snapshot, &Request<'_>) -> T,
    ) -> T {
        self.with_snapshot(|snapshot| {
            let primary_name = snapshot.primary_name();
            let request = Request {
                surface,
                current,
                service: snapshot
                    .services
                    .services
                    .iter()
                    .find(|candidate| candidate.id == service),
                services: &snapshot.services.services,
                primary_name: &primary_name,
                chosen: &snapshot.chosen,
            };
            build(snapshot, &request)
        })
    }
}
