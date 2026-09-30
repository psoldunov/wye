//! `GetApps`: the app chooser's list (DLG-APP-01 to DLG-APP-04).

use serde::{Deserialize, Serialize};

use crate::Packaging;

/// Installed apps and the recent link sources.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppList {
    /// Desktop entries, browsers only unless `GetApps(true)`.
    pub apps: Vec<AppInfo>,
    /// Desktop IDs of apps that recently sent links, newest first
    /// (DLG-APP-02).
    pub recent_sources: Vec<String>,
}

/// One desktop entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppInfo {
    /// Desktop ID.
    pub id: String,
    /// Localised `Name`.
    pub name: String,
    /// Localised `GenericName`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_name: Option<String>,
    /// `Keywords`, for search.
    pub keywords: Vec<String>,
    /// Icon theme name or absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// How the app is installed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packaging: Option<Packaging>,
    /// The entry handles `http`/`https` links.
    pub is_browser: bool,
}
