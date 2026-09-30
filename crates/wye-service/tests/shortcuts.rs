//! `GetShortcuts`, `SetShortcut`, `ConfigureShortcuts` and `ToggleMenu` on a
//! private bus with fake shortcuts and a fake UI host (KEY-40, KEY-41,
//! ADV-05 to ADV-07, TRAY-08). Skips without `dbus-daemon`.

#![allow(
    clippy::used_underscore_binding,
    reason = "the fake ignores some arguments; zbus's generated dispatch still passes them"
)]

mod support;

use std::sync::{Arc, Mutex};

use futures_lite::StreamExt as _;
use support::{Service, eventually};
use wye_api::Error;
use wye_api::names::{UI_BUS_NAME, UI_OBJECT_PATH};
use wye_api::picker::Placement;
use wye_api::shortcuts::{
    CLIPBOARD_ALTERNATIVE, CLIPBOARD_PRIMARY, ShortcutMechanism, Shortcuts, TOGGLE_MENU,
};
use wye_api::tray::TrayMenu;
use wye_service::platform::ShortcutProvider as _;

const BROWSERS: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n\
                        alternative = { app = \"fake-two.desktop\" }\n";

/// What the fake UI host was asked to show.
#[derive(Clone, Default)]
struct Menus(Arc<Mutex<Vec<String>>>);

impl Menus {
    fn shown(&self) -> Vec<String> {
        self.0.lock().expect("not poisoned").clone()
    }
}

struct PickerHost(Menus);

#[allow(
    clippy::unused_self,
    reason = "the fake serves the whole interface; members it ignores still take their arguments"
)]
#[zbus::interface(name = "dev.soldunov.wye.PickerHost1")]
impl PickerHost {
    fn show_picker(&self, _request_id: &str, _request: &str) {}

    fn close_picker(&self, _request_id: &str) {}

    fn show_menu(&self, menu: &str) {
        self.0.0.lock().expect("not poisoned").push(menu.to_owned());
    }
}

async fn fake_ui(service: &Service) -> (Menus, zbus::Connection) {
    let menus = Menus::default();
    let connection = service.bus.connect().await;
    connection
        .object_server()
        .at(UI_OBJECT_PATH, PickerHost(menus.clone()))
        .await
        .expect("served");
    connection.request_name(UI_BUS_NAME).await.expect("name");
    (menus, connection)
}

async fn shortcuts(service: &Service) -> Shortcuts {
    let text = service
        .wye()
        .await
        .get_shortcuts()
        .await
        .expect("GetShortcuts");
    wye_api::json::decode("shortcuts", &text).expect("Shortcuts JSON")
}

fn trigger(shortcuts: &Shortcuts, action: &str) -> Option<String> {
    shortcuts
        .bindings
        .iter()
        .find(|binding| binding.action == action)
        .and_then(|binding| binding.trigger.clone())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key41_every_action_is_listed_with_its_command() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let shortcuts = shortcuts(&service).await;
    // The fake owns its bindings the way the portal does.
    assert_eq!(shortcuts.mechanism, ShortcutMechanism::Portal);
    let listed: Vec<_> = shortcuts
        .bindings
        .iter()
        .map(|binding| (binding.action.as_str(), binding.command.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            (TOGGLE_MENU, "wye menu"),
            (CLIPBOARD_PRIMARY, "wye clipboard"),
            (CLIPBOARD_ALTERNATIVE, "wye clipboard --alternative"),
        ]
    );
    assert!(shortcuts.bindings.iter().all(|b| b.trigger.is_none()));
    assert!(shortcuts.bindings.iter().all(|b| !b.description.is_empty()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv05_set_shortcut_saves_the_preference_and_binds_it() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    wye.set_shortcut(TOGGLE_MENU, " control+alt+W ")
        .await
        .expect("set");
    let saved = service.desktop.read("config/wye/config.toml");
    assert!(
        saved.contains("toggle-menu = \"Ctrl+Alt+w\""),
        "saved in canonical form: {saved}"
    );
    assert_eq!(
        trigger(&shortcuts(&service).await, TOGGLE_MENU).as_deref(),
        Some("Ctrl+Alt+w")
    );

    wye.set_shortcut(TOGGLE_MENU, "").await.expect("cleared");
    let saved = service.desktop.read("config/wye/config.toml");
    assert!(!saved.contains("toggle-menu"), "cleared: {saved}");
    assert_eq!(trigger(&shortcuts(&service).await, TOGGLE_MENU), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_shortcut_refuses_unknown_actions_and_bindings() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    assert!(matches!(
        wye.set_shortcut("paste", "Ctrl+v").await,
        Err(Error::InvalidArgs(_))
    ));
    assert!(matches!(
        wye.set_shortcut(CLIPBOARD_PRIMARY, "Hyper+v").await,
        Err(Error::InvalidArgs(_))
    ));
    let bound = service.fakes.shortcuts.bindings().await.expect("listed");
    assert!(bound.is_empty(), "nothing bound: {bound:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key40_change_opens_the_mechanisms_dialog() {
    let Some(service) = Service::start("").await else {
        return;
    };
    service
        .wye()
        .await
        .configure_shortcuts()
        .await
        .expect("configured");
    assert_eq!(service.fakes.shortcuts.configured(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tray08_toggle_menu_shows_the_popup_at_the_pointer() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let (menus, _ui) = fake_ui(&service).await;
    let wye = service.wye().await;
    let mut requested = wye.receive_menu_requested().await.expect("subscribed");
    service.fakes.pointer.set(Some(Placement {
        output: "DP-1".into(),
        x: 100,
        y: 200,
    }));

    wye.toggle_menu().await.expect("toggled");

    let shown = menus.shown();
    assert_eq!(shown.len(), 1);
    let menu: TrayMenu = wye_api::json::decode("menu", &shown[0]).expect("a tray menu");
    assert!(!menu.items.is_empty());
    let payload: serde_json::Value = serde_json::from_str(&shown[0]).expect("JSON");
    assert_eq!(
        payload["placement"],
        serde_json::json!({"output": "DP-1", "x": 100, "y": 200})
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), requested.next())
        .await
        .expect("MenuRequested in time")
        .expect("MenuRequested");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tray08_toggle_menu_without_a_ui_host_is_unavailable() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let result = service.wye().await.toggle_menu().await;
    assert!(matches!(result, Err(Error::Unavailable(_))), "{result:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs run::start to start api::shortcuts::spawn_tasks (U13 needs in run.rs)"]
async fn adv05_pressing_toggle_menu_shows_the_popup() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let (menus, _ui) = fake_ui(&service).await;
    service.fakes.shortcuts.press(TOGGLE_MENU);
    eventually("the popup", || async { menus.shown().len() == 1 }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs run::start to start api::shortcuts::spawn_tasks (U13 needs in run.rs)"]
async fn adv06_adv07_pressing_a_clipboard_shortcut_opens_the_link() {
    let Some(service) = Service::start(BROWSERS).await else {
        return;
    };
    service.fakes.clipboard.copy("https://example.com/");
    service.fakes.shortcuts.press(CLIPBOARD_PRIMARY);
    eventually("the primary browser", || async {
        service.launched().len() == 1
    })
    .await;
    service.fakes.shortcuts.press(CLIPBOARD_ALTERNATIVE);
    eventually("the alternative browser", || async {
        service.launched().len() == 2
    })
    .await;
    let programs: Vec<_> = service
        .launched()
        .into_iter()
        .map(|argv| argv[0].clone())
        .collect();
    assert_eq!(programs, ["fake-one", "fake-two"]);
}
