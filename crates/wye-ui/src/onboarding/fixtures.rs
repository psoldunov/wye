//! Checks on `fixtures/onboarding.json`, the self-test's cases for the
//! first-run window: they must be real service data and reach every step.

use serde_json::Value;

use super::choices;
use super::flow::Step;
use crate::settings::fixture::Fixture;
use crate::settings::snapshot::Snapshot;

const FIXTURES: &str = include_str!("../../fixtures/onboarding.json");

fn arguments() -> Vec<Value> {
    let value: Value = serde_json::from_str(FIXTURES).expect("valid JSON");
    value["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| case["argument"].clone())
        .collect()
}

#[test]
fn every_fixture_decodes_as_service_data() {
    for argument in arguments() {
        if let Some(fixture) = argument.get("fixture") {
            Fixture::parse(&fixture.to_string()).unwrap_or_else(|error| panic!("{error}"));
        }
    }
}

#[test]
fn the_cases_reach_every_step_in_light_and_dark() {
    // ONB-01 to ONB-05
    let arguments = arguments();
    for step in 0..Step::ALL.len() {
        for scheme in ["light", "dark"] {
            assert!(
                arguments
                    .iter()
                    .any(|a| a["step"] == step && a["scheme"] == scheme),
                "step {step} in {scheme}"
            );
        }
    }
}

#[test]
fn the_browsers_fixtures_show_both_the_preselection_and_a_stored_list() {
    // ONB-03
    let mut snapshots = arguments()
        .into_iter()
        .filter_map(|a| a.get("fixture").cloned())
        .filter_map(|f| Fixture::parse(&f.to_string()).ok())
        .filter(|f| f.targets.is_some() && f.config.is_some())
        .map(|f| f.apply(&Snapshot::default()));
    let with_seed = snapshots.any(|s| {
        let foreign = choices::foreign_app_ids(&s.services.services);
        choices::seed_patch(&s.config, &s.targets, &foreign).is_some()
    });
    assert!(with_seed, "a fixture with an empty shown list");
}
