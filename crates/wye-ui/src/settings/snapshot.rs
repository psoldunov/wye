//! What the service said about the configuration and the machine, as the
//! Settings window holds it. Values are immutable: a change returns a new
//! [`Snapshot`].

use serde_json::Value;
use wye_api::Error;
use wye_api::apps::AppList;
use wye_api::json;
use wye_api::services::{ServiceInfo, ServiceList};
use wye_api::status::Status;
use wye_api::targets::TargetInventory;
use wye_core::Config;
use wye_core::merge_patch;

/// Everything the pages read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// The configuration as `GetConfig` returned it (kebab-case JSON).
    pub config: Value,
    /// Its revision, the `base_revision` of the next `UpdateConfig`.
    pub revision: u64,
    /// The `Status` property.
    pub status: Status,
    /// `GetTargets`.
    pub targets: TargetInventory,
    /// `GetServices`.
    pub services: ServiceList,
}

impl Snapshot {
    /// Read the configuration and its revision.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when `text` is not JSON.
    pub fn with_config(&self, text: &str, revision: u64) -> Result<Self, Error> {
        Ok(Self {
            config: json::decode("configuration", text)?,
            revision,
            ..self.clone()
        }
        .with_service_targets())
    }

    /// Read the `Status` property.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when `text` is not a status.
    pub fn with_status(&self, text: &str) -> Result<Self, Error> {
        Ok(Self {
            status: json::decode("status", text)?,
            ..self.clone()
        })
    }

    /// Read `GetTargets` and `GetServices`.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when either is not what it should be.
    pub fn with_inventory(&self, targets: &str, services: &str) -> Result<Self, Error> {
        Ok(Self {
            targets: json::decode("targets", targets)?,
            services: json::decode("services", services)?,
            ..self.clone()
        }
        .with_service_targets())
    }

    /// The configuration after `patch`, with the revision the service
    /// returned. Used for the answer to `UpdateConfig` and, before it, to
    /// show the change at once.
    #[must_use]
    pub fn with_patch(&self, patch: &Value, revision: u64) -> Self {
        Self {
            config: merge_patch::apply(&self.config, patch),
            revision,
            ..self.clone()
        }
        .with_service_targets()
    }

    /// The services with the target the configuration maps each one to
    /// (`apps.<id>`, absent for Default: APP-04, APP-06). `GetServices`
    /// reports the same, but it is read again only when the inventory
    /// revision moves: without this, a row on the Apps page kept its old
    /// target after a change until the window was reopened. Before
    /// `GetConfig` answers (or when it failed) there is no configuration
    /// to read, and each service keeps the target `GetServices` gave it.
    #[must_use]
    pub fn with_service_targets(self) -> Self {
        if !self.config.is_object() {
            return self;
        }
        let apps = self.config.get("apps").and_then(Value::as_object);
        let services = self
            .services
            .services
            .iter()
            .map(|service| ServiceInfo {
                target: apps
                    .and_then(|apps| apps.get(&service.id))
                    .filter(|target| !target.is_null())
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"default": true})),
                ..service.clone()
            })
            .collect();
        Self {
            services: ServiceList { services },
            ..self
        }
    }

    /// `Status.uiState` after `patch` (`UpdateUiState`).
    #[must_use]
    pub fn with_ui_state_patch(&self, patch: &Value) -> Self {
        let current = serde_json::to_value(&self.status.ui_state).unwrap_or(Value::Null);
        let merged = merge_patch::apply(&current, patch);
        let ui_state = serde_json::from_value(merged).unwrap_or_else(|error| {
            tracing::warn!(%error, "ignored an unreadable UI state patch");
            self.status.ui_state.clone()
        });
        Self {
            status: Status {
                ui_state,
                ..self.status.clone()
            },
            ..self.clone()
        }
    }

    /// The configuration as the core reads it. A configuration the core
    /// cannot read (the service validates, so this is a mismatch of
    /// versions) gives the defaults.
    #[must_use]
    pub fn typed_config(&self) -> Config {
        serde_json::from_value(self.config.clone()).unwrap_or_default()
    }

    /// Whether changes can be saved. False for a read-only file such as
    /// home-manager's (`Status.config.writable`).
    #[must_use]
    pub const fn writable(&self) -> bool {
        self.status.config.writable
    }

    /// Whether the session reports held modifiers (KEY-06).
    #[must_use]
    pub const fn held_keys_available(&self) -> bool {
        self.status.capabilities.held_keys.is_some()
    }

    /// Whether the session can watch the clipboard (EXT-12): the rows that
    /// rewrite copied text need it.
    #[must_use]
    pub const fn clipboard_watch_available(&self) -> bool {
        self.status.capabilities.clipboard_watch.is_some()
    }

    /// Whether the user dismissed callout `id` (BLK-09).
    #[must_use]
    pub fn callout_dismissed(&self, id: &str) -> bool {
        self.status
            .ui_state
            .dismissed_callouts
            .iter()
            .any(|dismissed| dismissed == id)
    }

    /// The name of the primary browser, for "Default (<primary>)" (APP-04).
    #[must_use]
    pub fn primary_name(&self) -> String {
        let primary = self
            .config
            .pointer("/browsers/primary")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"picker": true}));
        self.targets
            .targets
            .iter()
            .find(|info| info.target == primary)
            .map_or_else(
                || wye_core::target_menu::PICKER_LABEL.to_owned(),
                |info| info.name.clone(),
            )
    }
}

/// `GetApps` decoded (DLG-APP).
///
/// # Errors
///
/// `InvalidArgs` when `text` is not an app list.
pub fn decode_apps(text: &str) -> Result<AppList, Error> {
    json::decode("apps", text)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wye_api::status::ConfigStatus;

    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot::default()
            .with_config(
                r#"{"browsers":{"primary":{"app":"firefox.desktop"}},"general":{"show-tray-icon":true}}"#,
                3,
            )
            .expect("config")
    }

    #[test]
    fn a_patch_changes_the_configuration_and_the_revision() {
        let next = snapshot().with_patch(&json!({"general": {"show-tray-icon": false}}), 4);
        assert_eq!(next.revision, 4);
        assert_eq!(next.config["general"]["show-tray-icon"], json!(false));
        assert_eq!(
            next.config["browsers"]["primary"],
            json!({"app": "firefox.desktop"})
        );
    }

    #[test]
    fn the_original_is_not_changed_by_a_patch() {
        let before = snapshot();
        let _ = before.with_patch(&json!({"general": null}), 9);
        assert_eq!(before.revision, 3);
        assert!(before.config.get("general").is_some());
    }

    #[test]
    fn a_read_only_file_is_not_writable() {
        // SET-06: home-manager's file
        let status = Status {
            config: ConfigStatus {
                writable: false,
                ..ConfigStatus::default()
            },
            ..Status::default()
        };
        let text = serde_json::to_string(&status).expect("json");
        assert!(!snapshot().with_status(&text).expect("status").writable());
    }

    #[test]
    fn a_dismissed_callout_is_remembered_locally() {
        // BLK-09
        let next = snapshot().with_ui_state_patch(&json!({"dismissedCallouts": ["general-links"]}));
        assert!(next.callout_dismissed("general-links"));
        assert!(!next.callout_dismissed("apps-read-first"));
    }

    #[test]
    fn the_primary_name_follows_the_configuration() {
        // APP-04
        let targets = json!({"targets": [
            {"target": {"picker": true}, "kind": "picker", "name": "Picker"},
            {"target": {"app": "firefox.desktop"}, "kind": "app", "name": "Firefox"}
        ]})
        .to_string();
        let next = snapshot()
            .with_inventory(&targets, "{}")
            .expect("inventory");
        assert_eq!(next.primary_name(), "Firefox");
        let picker = next.with_patch(
            &json!({"browsers": {"primary": {"app": null, "picker": true}}}),
            5,
        );
        assert_eq!(picker.primary_name(), "Picker");
    }

    #[test]
    fn held_keys_are_available_when_a_mechanism_is_reported() {
        // KEY-06
        let mut status = Status::default();
        assert!(!snapshot().held_keys_available());
        status.capabilities.held_keys = Some("wayland-layer-shell".to_owned());
        let text = serde_json::to_string(&status).expect("json");
        assert!(
            snapshot()
                .with_status(&text)
                .expect("status")
                .held_keys_available()
        );
    }

    #[test]
    fn a_service_follows_its_mapping_at_once() {
        // APP-04, APP-06: the Apps page row shows the new target without a
        // new `GetServices`; Default is not stored, and an ID with a dot is
        // one key.
        let services = json!({"services": [
            {"id": "discord", "name": "Discord", "target": {"default": true}},
            {"id": "x.com", "name": "X", "target": {"app": "firefox.desktop"}}
        ]})
        .to_string();
        let base = snapshot()
            .with_inventory(r#"{"targets": []}"#, &services)
            .expect("inventory");
        assert_eq!(base.services.services[1].target, json!({"default": true}));
        let mapped = base.with_patch(
            &json!({"apps": {"discord": {"app": "com.discordapp.Discord.desktop"}, "x.com": {"private": "firefox.desktop"}}}),
            4,
        );
        assert_eq!(
            mapped.services.services[0].target,
            json!({"app": "com.discordapp.Discord.desktop"})
        );
        assert_eq!(
            mapped.services.services[1].target,
            json!({"private": "firefox.desktop"})
        );
        let back = mapped.with_patch(&json!({"apps": {"discord": null}}), 5);
        assert_eq!(back.services.services[0].target, json!({"default": true}));
        let reread = back
            .with_config(r#"{"apps": {"discord": {"app": "vesktop.desktop"}}}"#, 6)
            .expect("config");
        assert_eq!(
            reread.services.services[0].target,
            json!({"app": "vesktop.desktop"})
        );
    }

    #[test]
    fn a_service_keeps_its_own_target_without_a_configuration() {
        // APP-04: before `GetConfig` answers, or when it failed, the
        // configuration is not an object; `GetServices` is all there is.
        let services = json!({"services": [
            {"id": "discord", "name": "Discord", "target": {"app": "vesktop.desktop"}}
        ]})
        .to_string();
        let early = Snapshot::default()
            .with_inventory(r#"{"targets": []}"#, &services)
            .expect("inventory");
        assert_eq!(early.config, Value::Null);
        assert_eq!(
            early.services.services[0].target,
            json!({"app": "vesktop.desktop"})
        );
        let loaded = early.with_config("{}", 1).expect("config");
        assert_eq!(loaded.services.services[0].target, json!({"default": true}));
    }

    #[test]
    fn a_broken_payload_is_an_error() {
        assert!(snapshot().with_status("{").is_err());
        assert!(snapshot().with_config("[", 1).is_err());
        assert!(decode_apps("nope").is_err());
    }
}
