//! `GetServices`: the web app catalogue with what is installed (APP-03,
//! APP-05).

use serde::{Deserialize, Serialize};

use crate::{AppRef, TargetSpec};

/// The catalogue as the Apps page shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceList {
    /// Services in catalogue order.
    pub services: Vec<ServiceInfo>,
}

/// One web service.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceInfo {
    /// Catalogue ID, for example `zoom`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Icon theme name or absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The service's own desktop app, when installed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_app: Option<AppRef>,
    /// The configured mapping; `{"default": true}` when none is stored.
    pub target: TargetSpec,
}
