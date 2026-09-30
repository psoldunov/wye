//! A picker request for the tests: Firefox (hotkey `f`, private windows),
//! a Chrome profile (hotkey `2`, badge), and Brave in the Open In menu.

use serde_json::json;

use super::view::PickerView;

/// The request as the service sends it.
pub fn request() -> serde_json::Value {
    json!({
        "url": {
            "full": "https://github.com/psoldunov/wye?tab=readme",
            "host": "github.com",
            "rest": "/psoldunov/wye?tab=readme"
        },
        "source": { "name": "Slack", "icon": "slack" },
        "tiles": [
            {
                "target": { "app": "firefox.desktop" },
                "name": "Firefox",
                "icon": "firefox",
                "hotkey": "f",
                "capabilities": { "private": true, "newWindow": true, "background": true }
            },
            {
                "target": { "profile": { "app": "google-chrome.desktop", "id": "Profile 1" } },
                "name": "Work",
                "icon": "google-chrome",
                "badge": { "initial": "W", "color": "#336699" },
                "hotkey": "2",
                "capabilities": { "private": false, "newWindow": true, "background": true }
            }
        ],
        "overflow": [
            {
                "label": "Browsers",
                "tiles": [{
                    "target": { "app": "brave.desktop" },
                    "name": "Brave",
                    "icon": "brave",
                    "capabilities": { "private": true, "newWindow": true, "background": true }
                }]
            }
        ],
        "settings": { "iconSize": "large", "showNames": true, "showUrl": true, "showBadge": true },
        "keys": {
            "actions": {
                "open": ["Return", "KP_Enter", "space"],
                "cancel": ["Escape"],
                "next": ["Right", "Tab"],
                "previous": ["Left", "Shift+Tab"],
                "first": ["Home"],
                "last": ["End"],
                "copy-link": ["Ctrl+c"],
                "more": ["Menu"],
                "create-rule": ["Ctrl+r"]
            },
            "modifierActions": {
                "private": ["Shift"],
                "background": ["Ctrl"],
                "new-window": ["Alt"]
            }
        },
        "held": [],
        "preview": false
    })
}

/// The view of [`request`].
pub fn view() -> PickerView {
    PickerView::parse(&request().to_string()).expect("the fixture is a valid request")
}
