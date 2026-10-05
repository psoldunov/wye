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
    /// The service's own desktop app, when installed: the first of
    /// `installed_apps`, kept for older readers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_app: Option<AppRef>,
    /// Every installed own app (APP-05, APP-12): the catalogue's desktop IDs
    /// first, then the apps named after the service.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub installed_apps: Vec<AppRef>,
    /// The configured mapping; `{"default": true}` when none is stored.
    pub target: TargetSpec,
}

impl ServiceInfo {
    /// Every installed own app, the preferred one first (APP-05, APP-12). A
    /// service that predates `installed_apps` sends only `installed_app`.
    #[must_use]
    pub fn own_apps(&self) -> &[AppRef] {
        if self.installed_apps.is_empty() {
            self.installed_app.as_slice()
        } else {
            &self.installed_apps
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> AppRef {
        AppRef {
            id: id.to_owned(),
            name: id.to_owned(),
            icon: None,
        }
    }

    // APP-12
    #[test]
    fn app12_installed_apps_travel_as_installed_apps() {
        let info = ServiceInfo {
            installed_apps: vec![app("a.desktop"), app("b.desktop")],
            ..ServiceInfo::default()
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["installedApps"].as_array().unwrap().len(), 2);
        let back: ServiceInfo = serde_json::from_value(json).unwrap();
        assert_eq!(back.own_apps(), info.installed_apps.as_slice());
        assert!(
            serde_json::to_value(ServiceInfo::default())
                .unwrap()
                .get("installedApps")
                .is_none()
        );
    }

    // APP-05
    #[test]
    fn own_apps_fall_back_to_installed_app_for_an_older_service() {
        let info: ServiceInfo =
            serde_json::from_str(r#"{"id":"x","installedApp":{"id":"a.desktop","name":"A"}}"#)
                .unwrap();
        assert_eq!(info.own_apps().len(), 1);
        assert_eq!(info.own_apps()[0].id, "a.desktop");
        assert!(ServiceInfo::default().own_apps().is_empty());
    }
}
