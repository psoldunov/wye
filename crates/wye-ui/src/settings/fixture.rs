//! The self-test's stand-in for the service: `fixtures/settings.json`
//! carries what the service would have returned, and the window shows it
//! without a bus (`SettingsBackend.loadFixture`).

use serde::Deserialize;
use serde_json::Value;
use wye_api::apps::AppList;
use wye_api::expansion::ExpansionCatalogue;
use wye_api::services::ServiceList;
use wye_api::shortcuts::Shortcuts;
use wye_api::status::Status;
use wye_api::targets::TargetInventory;

use super::snapshot::Snapshot;

/// The revision a fixture reports when it names none.
const DEFAULT_REVISION: u64 = 1;

/// What a `fixture` case holds. Every part is optional: a later fixture
/// changes only the parts it names (a read-only `status`, say) and keeps the
/// rest of what the window shows.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fixture {
    /// `GetConfig`, in the configuration's kebab-case shape.
    #[serde(default)]
    pub config: Option<Value>,
    #[serde(default)]
    pub revision: Option<u64>,
    /// `Status`.
    #[serde(default)]
    pub status: Option<Status>,
    /// `GetTargets`.
    #[serde(default)]
    pub targets: Option<TargetInventory>,
    /// `GetServices`.
    #[serde(default)]
    pub services: Option<ServiceList>,
    /// `GetApps(true)`.
    #[serde(default)]
    pub apps: Option<AppList>,
    /// `GetShortcuts`.
    #[serde(default)]
    pub shortcuts: Option<Shortcuts>,
    /// `GetExpansionCatalogue`.
    #[serde(default)]
    pub expansion: Option<ExpansionCatalogue>,
}

impl Fixture {
    /// Read a fixture from its JSON text.
    ///
    /// # Errors
    ///
    /// The serde error, naming the field, when the text is not a fixture.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// `base` with the parts this fixture names replaced.
    #[must_use]
    pub fn apply(&self, base: &Snapshot) -> Snapshot {
        Snapshot {
            config: self.config.clone().unwrap_or_else(|| base.config.clone()),
            revision: self.revision.unwrap_or(if base.revision == 0 {
                DEFAULT_REVISION
            } else {
                base.revision
            }),
            status: self.status.clone().unwrap_or_else(|| base.status.clone()),
            targets: self.targets.clone().unwrap_or_else(|| base.targets.clone()),
            services: self
                .services
                .clone()
                .unwrap_or_else(|| base.services.clone()),
            chosen: base.chosen.clone(),
        }
        .with_service_targets()
    }
}

/// The `fixture` of every case in the text of a `fixtures/<surface>.json`
/// file. Shared by the fixture tests of the other surfaces.
#[cfg(test)]
pub(crate) fn case_fixtures(text: &str) -> Vec<Value> {
    let value: Value = serde_json::from_str(text).expect("valid JSON");
    value["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .filter_map(|case| case["argument"].get("fixture").cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: &str = include_str!("../../fixtures/settings.json");

    /// The `fixture` of every case of the settings fixture file.
    ///
    /// A case's `argument` is a page name or an object with `page`,
    /// `fixture`, `scheme` and `sheet` (`qml/settings/SettingsWindow.qml`).
    fn fixtures() -> Vec<Value> {
        case_fixtures(SETTINGS)
    }

    #[test]
    fn every_settings_fixture_decodes_as_service_data() {
        let cases = fixtures();
        assert!(!cases.is_empty(), "the settings fixture has no data");
        for fixture in cases {
            Fixture::parse(&fixture.to_string()).unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn every_settings_fixture_config_is_a_valid_configuration() {
        for fixture in fixtures() {
            let Some(config) = Fixture::parse(&fixture.to_string())
                .expect("fixture")
                .config
            else {
                continue;
            };
            serde_json::from_value::<wye_core::Config>(config)
                .unwrap_or_else(|error| panic!("{error}"));
        }
    }

    #[test]
    fn an_empty_fixture_changes_nothing_but_starts_at_a_revision() {
        let base = Snapshot::default();
        let snapshot = Fixture::parse("{}").expect("fixture").apply(&base);
        assert_eq!(snapshot.config, base.config);
        assert_eq!(snapshot.revision, DEFAULT_REVISION);
    }

    #[test]
    fn a_later_fixture_keeps_what_it_does_not_name() {
        let first = Fixture::parse(r#"{"config": {"general": {}}, "revision": 4}"#)
            .expect("fixture")
            .apply(&Snapshot::default());
        let second = Fixture::parse(r#"{"status": {"config": {"writable": true}}}"#)
            .expect("fixture")
            .apply(&first);
        assert_eq!(second.config, first.config);
        assert_eq!(second.revision, 4);
        assert!(second.writable());
    }

    #[test]
    fn gen_01_the_fixture_cases_show_nix_starting_wye_and_not() {
        // Both first-run and Settings fixtures carry `loginManagedOn`
        // through the typed `Status`, so the "on" cases render as on.
        for file in ["settings.json", "onboarding.json"] {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/");
            let text = std::fs::read_to_string(format!("{path}{file}")).expect("fixture file");
            let cases: serde_json::Value = serde_json::from_str(&text).expect("json");
            let managed: Vec<bool> = cases["cases"]
                .as_array()
                .expect("cases")
                .iter()
                .filter_map(|case| case["argument"]["fixture"].as_object())
                .filter(|fixture| fixture.contains_key("status"))
                .map(|fixture| {
                    let text = serde_json::Value::Object(fixture.clone()).to_string();
                    Fixture::parse(&text)
                        .expect("fixture")
                        .apply(&Snapshot::default())
                        .status
                })
                .filter(|status| status.login_managed)
                .map(|status| status.login_managed_on)
                .collect();
            assert_eq!(managed, [false, true], "{file}");
        }
    }
}
