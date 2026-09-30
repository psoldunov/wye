//! DLG-EXP-03: the redirect chain of a short link.

use std::cell::RefCell;
use std::collections::HashMap;

use super::*;
use crate::hooks::Location;

/// Answers from a table and records every link it was asked about.
struct Fake {
    answers: HashMap<String, Result<Option<Location>, ResolveError>>,
    asked: RefCell<Vec<String>>,
}

impl Fake {
    fn new(answers: &[(&str, Result<Option<&str>, ResolveError>)]) -> Self {
        Self {
            answers: answers
                .iter()
                .map(|(url, answer)| {
                    (
                        (*url).to_owned(),
                        answer.clone().map(|l| l.map(Location::new)),
                    )
                })
                .collect(),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

impl ShortLinkResolver for Fake {
    fn resolve(&self, url: &Url) -> Result<Option<Location>, ResolveError> {
        self.asked.borrow_mut().push(url.to_string());
        self.answers.get(url.as_str()).cloned().unwrap_or(Ok(None))
    }
}

fn expand(start: &str, settings: &ExpansionSettings, fake: &Fake) -> Expanded {
    ExpansionCatalogue::shipped().expand_short_link(&Url::parse(start).unwrap(), settings, fake)
}

fn hops(expanded: &Expanded) -> Vec<String> {
    expanded.hops.iter().map(ToString::to_string).collect()
}

#[test]
fn a_location_on_another_host_is_the_end_and_is_not_contacted() {
    let fake = Fake::new(&[(
        "https://bit.ly/abc",
        Ok(Some("https://example.com/page?x=1")),
    )]);
    let expanded = expand("https://bit.ly/abc", &ExpansionSettings::default(), &fake);
    assert_eq!(hops(&expanded), ["https://example.com/page?x=1"]);
    assert_eq!(expanded.stop, ExpandStop::Destination);
    assert_eq!(expanded.stop.problem(), None);
    // DLG-EXP-03: example.com is not an enabled short link, so it is never
    // contacted; it was learned from bit.ly's answer.
    assert_eq!(fake.asked(), ["https://bit.ly/abc"]);
}

#[test]
fn every_hop_that_is_resolved_is_an_enabled_short_link() {
    let fake = Fake::new(&[
        ("https://bit.ly/abc", Ok(Some("https://t.co/x"))),
        ("https://t.co/x", Ok(Some("https://example.com/final"))),
    ]);
    let expanded = expand("https://bit.ly/abc", &ExpansionSettings::default(), &fake);
    assert_eq!(
        hops(&expanded),
        ["https://t.co/x", "https://example.com/final"]
    );
    assert_eq!(expanded.stop, ExpandStop::Destination);
    assert_eq!(fake.asked(), ["https://bit.ly/abc", "https://t.co/x"]);
}

#[test]
fn a_disabled_short_link_hop_is_not_contacted() {
    let fake = Fake::new(&[
        ("https://bit.ly/abc", Ok(Some("https://t.co/x"))),
        ("https://t.co/x", Ok(Some("https://example.com/final"))),
    ]);
    let settings = ExpansionSettings {
        disabled: vec!["t.co".into()],
        ..ExpansionSettings::default()
    };
    let expanded = expand("https://bit.ly/abc", &settings, &fake);
    assert_eq!(hops(&expanded), ["https://t.co/x"]);
    assert_eq!(expanded.last().map(Url::as_str), Some("https://t.co/x"));
    assert_eq!(expanded.stop, ExpandStop::Destination);
    assert_eq!(fake.asked(), ["https://bit.ly/abc"]);
}

#[test]
fn a_non_redirect_answer_ends_at_the_start() {
    let fake = Fake::new(&[("https://bit.ly/abc", Ok(None))]);
    let expanded = expand("https://bit.ly/abc", &ExpansionSettings::default(), &fake);
    assert!(expanded.hops.is_empty());
    assert_eq!(expanded.last(), None);
    assert_eq!(expanded.stop, ExpandStop::Destination);
}

#[test]
fn resolves_relative_locations() {
    let fake = Fake::new(&[
        ("https://bit.ly/abc", Ok(Some("/next/hop"))),
        ("https://bit.ly/next/hop", Ok(Some("//t.co/final"))),
        ("https://t.co/final", Ok(Some("sub/page"))),
    ]);
    let expanded = expand("https://bit.ly/abc", &ExpansionSettings::default(), &fake);
    assert_eq!(
        hops(&expanded),
        [
            "https://bit.ly/next/hop",
            "https://t.co/final",
            "https://t.co/sub/page"
        ]
    );
}

#[test]
fn only_an_enabled_short_link_is_ever_contacted_first() {
    let fake = Fake::new(&[]);
    let settings = ExpansionSettings {
        disabled: vec!["bit.ly".into()],
        ..ExpansionSettings::default()
    };
    let disabled = expand("https://bit.ly/abc", &settings, &fake);
    let unknown = expand("https://example.com/abc", &settings, &fake);
    assert_eq!(disabled.stop, ExpandStop::NotShortLink);
    assert_eq!(unknown.stop, ExpandStop::NotShortLink);
    assert!(fake.asked().is_empty(), "nothing may be contacted");
}

#[test]
fn custom_short_link_domains_are_followed() {
    let settings = ExpansionSettings {
        custom_short_links: vec!["go.example".into()],
        ..ExpansionSettings::default()
    };
    let fake = Fake::new(&[("https://go.example/x", Ok(Some("https://example.org/")))]);
    let expanded = expand("https://go.example/x", &settings, &fake);
    assert_eq!(hops(&expanded), ["https://example.org/"]);
}

#[test]
fn stops_at_the_redirect_limit() {
    let fake = Fake::new(&[
        ("https://bit.ly/a", Ok(Some("https://t.co/b"))),
        ("https://t.co/b", Ok(Some("https://tinyurl.com/c"))),
        ("https://tinyurl.com/c", Ok(Some("https://example.com/"))),
    ]);
    let settings = ExpansionSettings {
        max_redirects: 2,
        ..ExpansionSettings::default()
    };
    let expanded = expand("https://bit.ly/a", &settings, &fake);
    assert_eq!(hops(&expanded), ["https://t.co/b", "https://tinyurl.com/c"]);
    assert_eq!(expanded.stop, ExpandStop::LimitReached);
    assert_eq!(
        expanded.stop.problem().as_deref(),
        Some("too many redirects")
    );
    assert_eq!(fake.asked().len(), 2, "no request beyond the limit");
}

#[test]
fn a_chain_of_exactly_the_limit_is_not_probed_further() {
    let fake = Fake::new(&[("https://bit.ly/a", Ok(Some("https://t.co/b")))]);
    let settings = ExpansionSettings {
        max_redirects: 1,
        ..ExpansionSettings::default()
    };
    let expanded = expand("https://bit.ly/a", &settings, &fake);
    assert_eq!(hops(&expanded), ["https://t.co/b"]);
    assert_eq!(expanded.stop, ExpandStop::LimitReached);
    assert_eq!(fake.asked(), ["https://bit.ly/a"]);
}

#[test]
fn stops_before_a_non_web_location() {
    let fake = Fake::new(&[
        ("https://bit.ly/a", Ok(Some("https://t.co/b"))),
        ("https://t.co/b", Ok(Some("intent://open#Intent;end"))),
    ]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert_eq!(hops(&expanded), ["https://t.co/b"]);
    assert_eq!(expanded.stop, ExpandStop::NonWeb("intent".into()));
    assert!(expanded.stop.problem().unwrap().contains("intent:"));

    let fake = Fake::new(&[("https://bit.ly/a", Ok(Some("spotify:track:1")))]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert!(expanded.hops.is_empty());
    assert_eq!(expanded.stop, ExpandStop::NonWeb("spotify".into()));
}

#[test]
fn rejects_locations_that_are_not_links() {
    let fake = Fake::new(&[("https://bit.ly/a", Ok(Some("http://")))]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert!(expanded.hops.is_empty());
    assert!(matches!(expanded.stop, ExpandStop::BadLocation(_)));
    assert!(expanded.stop.problem().is_some());
}

#[test]
fn detects_loops() {
    let fake = Fake::new(&[
        ("https://bit.ly/a", Ok(Some("https://t.co/b"))),
        ("https://t.co/b", Ok(Some("https://bit.ly/a"))),
    ]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert_eq!(hops(&expanded), ["https://t.co/b"]);
    assert_eq!(expanded.stop, ExpandStop::Loop);

    let fake = Fake::new(&[("https://bit.ly/a", Ok(Some("/a")))]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert_eq!(expanded.stop, ExpandStop::Loop);
    assert!(expanded.hops.is_empty());
}

#[test]
fn keeps_the_progress_when_a_request_fails() {
    // PIPE-03: on a timeout the pipeline continues with what it has.
    let fake = Fake::new(&[
        ("https://bit.ly/a", Ok(Some("https://t.co/b"))),
        ("https://t.co/b", Err(ResolveError::Timeout)),
    ]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert_eq!(hops(&expanded), ["https://t.co/b"]);
    assert_eq!(expanded.stop, ExpandStop::Failed(ResolveError::Timeout));
    assert!(expanded.stop.problem().unwrap().contains("in time"));

    let fake = Fake::new(&[(
        "https://bit.ly/a",
        Err(ResolveError::Failed("refused".into())),
    )]);
    let expanded = expand("https://bit.ly/a", &ExpansionSettings::default(), &fake);
    assert!(expanded.hops.is_empty());
    assert_eq!(expanded.stop.problem().as_deref(), Some("refused"));
}
