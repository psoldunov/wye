//! `GetExpansionCatalogue`: redirect wrappers and short-link domains for the
//! URL expansion sheet (DLG-EXP-01 to DLG-EXP-05).

use serde::{Deserialize, Serialize};

/// What Wye can expand and what is switched on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExpansionCatalogue {
    /// Redirect wrappers, unwrapped locally (DLG-EXP-01).
    pub wrappers: Vec<Wrapper>,
    /// Short-link domains, resolved over the network (DLG-EXP-02).
    pub short_links: Vec<ShortLink>,
}

/// One redirect wrapper.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Wrapper {
    /// Catalogue ID, for example `google`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Hosts, as in `data/expansion.toml`.
    pub hosts: Vec<String>,
    /// Unwrapping is on.
    pub enabled: bool,
}

/// One short-link domain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortLink {
    /// The domain, for example `bit.ly`.
    pub domain: String,
    /// Wye may contact it (DLG-EXP-03).
    pub enabled: bool,
}
