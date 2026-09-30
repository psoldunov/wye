use wye_api::expansion::{ShortLink, Wrapper as WireWrapper};
use wye_core::Config;
use wye_core::merge_patch::apply_to_config;

use super::*;

fn wire() -> ExpansionCatalogue {
    ExpansionCatalogue {
        wrappers: vec![WireWrapper {
            id: "google-search".to_owned(),
            name: "Google search results".to_owned(),
            hosts: vec!["google.com/url".to_owned()],
            enabled: true,
        }],
        short_links: ["bit.ly", "t.co", "mine.link"]
            .into_iter()
            .map(|domain| ShortLink {
                domain: domain.to_owned(),
                enabled: true,
            })
            .collect(),
    }
}

fn catalogue() -> Catalogue {
    Catalogue::from_wire(&wire(), &["mine.link".to_owned()])
}

fn settings_with(disabled: &[&str], custom: &[&str]) -> ExpansionSettings {
    ExpansionSettings {
        disabled: disabled.iter().map(|s| (*s).to_owned()).collect(),
        custom_short_links: custom.iter().map(|s| (*s).to_owned()).collect(),
        ..ExpansionSettings::default()
    }
}

fn applied(patch: &Value) -> ExpansionSettings {
    apply_to_config(&Config::default(), patch)
        .expect("a valid configuration")
        .advanced
        .expansion
}

#[test]
fn the_shipped_short_links_leave_out_the_users_own() {
    assert_eq!(catalogue().short_links, ["bit.ly", "t.co"]);
}

#[test]
fn rows_list_wrappers_then_shipped_then_custom_domains() {
    // DLG-EXP-01, DLG-EXP-02
    let rows = rows(&catalogue(), &settings_with(&[], &["mine.link"]));
    let ids: Vec<_> = rows
        .iter()
        .map(|row| (row.kind, row.id.as_str(), row.removable))
        .collect();
    assert_eq!(
        ids,
        [
            (Kind::Wrapper, "google-search", false),
            (Kind::ShortLink, "bit.ly", false),
            (Kind::ShortLink, "t.co", false),
            (Kind::ShortLink, "mine.link", true),
        ]
    );
    assert_eq!(rows[0].detail, "google.com/url");
}

#[test]
fn everything_is_on_unless_listed_as_disabled() {
    let rows = rows(
        &catalogue(),
        &settings_with(&["t.co", "GOOGLE-SEARCH"], &[]),
    );
    let on: Vec<_> = rows
        .iter()
        .filter(|row| row.enabled)
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(on, ["bit.ly"]);
}

#[test]
fn turning_a_service_off_and_on_edits_the_disabled_list() {
    // DLG-EXP-01, DLG-EXP-02
    let off = applied(&toggle_patch(&settings_with(&[], &[]), "t.co", false));
    assert_eq!(off.disabled, ["t.co"]);
    let on = applied(&toggle_patch(&off, "T.CO", true));
    assert!(on.disabled.is_empty());
}

#[test]
fn turning_off_twice_lists_it_once() {
    let once = settings_with(&["t.co"], &[]);
    assert_eq!(
        applied(&toggle_patch(&once, "t.co", false)).disabled,
        ["t.co"]
    );
}

fn normal(typed: &str) -> Result<String, DomainError> {
    normalise_domain(typed, &catalogue(), &ExpansionSettings::default())
}

#[test]
fn a_domain_is_normalised() {
    // DLG-EXP-02
    assert_eq!(normal("  Example.LINK "), Ok("example.link".to_owned()));
    assert_eq!(
        normal("https://www.example.link/abc?x=1"),
        Ok("example.link".to_owned())
    );
}

#[test]
fn a_bad_domain_is_refused_with_a_reason() {
    assert_eq!(normal(" "), Err(DomainError::Empty));
    for bad in ["localhost", "a b.com", "-a.com", "a..com", "exa$mple.com"] {
        assert_eq!(
            normal(bad),
            Err(DomainError::Invalid(bad.to_owned())),
            "{bad}"
        );
    }
}

#[test]
fn a_domain_already_listed_is_refused() {
    let settings = settings_with(&[], &["mine.link"]);
    assert_eq!(
        normalise_domain("BIT.LY", &catalogue(), &settings),
        Err(DomainError::Listed("bit.ly".to_owned()))
    );
    assert_eq!(
        normalise_domain("mine.link", &catalogue(), &settings),
        Err(DomainError::Listed("mine.link".to_owned()))
    );
}

#[test]
fn adding_appends_the_domain() {
    let patch = add_patch(&settings_with(&[], &["a.link"]), "b.link");
    assert_eq!(applied(&patch).custom_short_links, ["a.link", "b.link"]);
}

#[test]
fn removing_forgets_the_domain_and_its_off_switch() {
    // DLG-EXP-02: custom domains can be removed
    let settings = settings_with(&["mine.link", "t.co"], &["mine.link", "other.link"]);
    let next = applied(&remove_patch(&settings, "mine.link"));
    assert_eq!(next.custom_short_links, ["other.link"]);
    assert_eq!(next.disabled, ["t.co"]);
}
