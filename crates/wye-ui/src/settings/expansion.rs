//! The URL expansion sheet (DLG-EXP-01 to DLG-EXP-05): the rows it lists
//! and every edit as a pure function.
//!
//! The service ships the wrappers and short-link domains
//! (`GetExpansionCatalogue`); the configuration holds only what differs:
//! `advanced.expansion.disabled` (wrapper IDs and domains turned off) and
//! `custom-short-links` (domains the user added). The rows come from both,
//! so a change shows without asking the service again.

use serde::Serialize;
use serde_json::{Value, json};
use wye_api::expansion::ExpansionCatalogue;
use wye_core::config::ExpansionSettings;

/// The shipped part of the catalogue: what was there before the user added
/// domains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalogue {
    pub wrappers: Vec<Wrapper>,
    pub short_links: Vec<String>,
}

/// One redirect wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrapper {
    pub id: String,
    pub name: String,
    pub hosts: Vec<String>,
}

impl Catalogue {
    /// The shipped entries of `wire`; `custom` are the user's domains that
    /// were already in the configuration when the service answered, so they
    /// are not mistaken for shipped ones.
    #[must_use]
    pub fn from_wire(wire: &ExpansionCatalogue, custom: &[String]) -> Self {
        Self {
            wrappers: wire
                .wrappers
                .iter()
                .map(|wrapper| Wrapper {
                    id: wrapper.id.clone(),
                    name: wrapper.name.clone(),
                    hosts: wrapper.hosts.clone(),
                })
                .collect(),
            short_links: wire
                .short_links
                .iter()
                .map(|link| link.domain.clone())
                .filter(|domain| !custom.iter().any(|c| c.eq_ignore_ascii_case(domain)))
                .collect(),
        }
    }
}

/// What a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Wrapper,
    ShortLink,
}

/// One row of the sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub kind: Kind,
    /// The wrapper ID or the domain: what `disabled` holds.
    pub id: String,
    pub label: String,
    /// The hosts a wrapper covers, dimmed under the label.
    pub detail: String,
    pub enabled: bool,
    /// A domain the user added (DLG-EXP-02).
    pub removable: bool,
}

/// The rows: wrappers (DLG-EXP-01), then short-link domains, shipped first
/// and the user's last (DLG-EXP-02).
#[must_use]
pub fn rows(catalogue: &Catalogue, settings: &ExpansionSettings) -> Vec<Row> {
    let wrappers = catalogue.wrappers.iter().map(|wrapper| Row {
        kind: Kind::Wrapper,
        id: wrapper.id.clone(),
        label: wrapper.name.clone(),
        detail: wrapper.hosts.join(", "),
        enabled: settings.is_enabled(&wrapper.id),
        removable: false,
    });
    let shipped = catalogue.short_links.iter().map(|domain| (domain, false));
    let custom = settings
        .custom_short_links
        .iter()
        .map(|domain| (domain, true));
    let short_links = shipped.chain(custom).map(|(domain, removable)| Row {
        kind: Kind::ShortLink,
        id: domain.clone(),
        label: domain.clone(),
        detail: String::new(),
        enabled: settings.is_enabled(domain),
        removable,
    });
    wrappers.chain(short_links).collect()
}

/// The patch that turns `id` on or off. The whole `disabled` list goes:
/// arrays replace.
#[must_use]
pub fn toggle_patch(settings: &ExpansionSettings, id: &str, enabled: bool) -> Value {
    let mut disabled: Vec<String> = settings
        .disabled
        .iter()
        .filter(|existing| !existing.eq_ignore_ascii_case(id))
        .cloned()
        .collect();
    if !enabled {
        disabled.push(id.to_owned());
    }
    json!({"advanced": {"expansion": {"disabled": disabled}}})
}

/// Why a domain cannot be added.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("Enter a domain such as example.link")]
    Empty,
    #[error("“{0}” is not a domain name")]
    Invalid(String),
    #[error("“{0}” is in the list already")]
    Listed(String),
}

fn valid_label(label: &str) -> bool {
    (1..=63).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A domain as the user typed it, normalised: lower case, without a scheme,
/// a path or a leading `www.` (DLG-EXP-02).
///
/// # Errors
///
/// [`DomainError`] when it is empty, not a domain name, or already listed.
pub fn normalise_domain(
    typed: &str,
    catalogue: &Catalogue,
    settings: &ExpansionSettings,
) -> Result<String, DomainError> {
    let lowered = typed.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return Err(DomainError::Empty);
    }
    let without_scheme = lowered
        .strip_prefix("https://")
        .or_else(|| lowered.strip_prefix("http://"))
        .unwrap_or(&lowered);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let domain = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .trim_end_matches('.');
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|label| valid_label(label)) {
        return Err(DomainError::Invalid(typed.trim().to_owned()));
    }
    let listed = catalogue
        .short_links
        .iter()
        .chain(&settings.custom_short_links)
        .any(|existing| existing.eq_ignore_ascii_case(domain));
    if listed {
        return Err(DomainError::Listed(domain.to_owned()));
    }
    Ok(domain.to_owned())
}

/// The patch that adds a normalised `domain` (DLG-EXP-02).
#[must_use]
pub fn add_patch(settings: &ExpansionSettings, domain: &str) -> Value {
    let mut custom = settings.custom_short_links.clone();
    custom.push(domain.to_owned());
    json!({"advanced": {"expansion": {"custom-short-links": custom}}})
}

/// The patch that removes a custom `domain`, and its entry in `disabled`.
#[must_use]
pub fn remove_patch(settings: &ExpansionSettings, domain: &str) -> Value {
    let keep = |list: &[String]| -> Vec<String> {
        list.iter()
            .filter(|existing| !existing.eq_ignore_ascii_case(domain))
            .cloned()
            .collect()
    };
    json!({"advanced": {"expansion": {
        "custom-short-links": keep(&settings.custom_short_links),
        "disabled": keep(&settings.disabled),
    }}})
}

#[cfg(test)]
mod tests;
