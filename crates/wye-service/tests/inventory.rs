//! Apps and targets on a private bus (TGT-02 to TGT-07, APP-05, APP-10,
//! DLG-APP, DLG-EXP, DISC-02, BRW-06). Skips without `dbus-daemon`.

mod support;

use support::{ONE, Service, TWO, eventually};
use wye_api::apps::AppList;
use wye_api::expansion::ExpansionCatalogue;
use wye_api::json;
use wye_api::services::ServiceList;
use wye_api::targets::{TargetInventory, TargetKind};

async fn targets(service: &Service) -> TargetInventory {
    let text = service.wye().await.get_targets().await.expect("GetTargets");
    json::decode("targets", &text).expect("TargetInventory")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tgt02_targets_start_with_the_picker_and_list_each_browser() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let inventory = targets(&service).await;
    assert_eq!(inventory.targets[0].kind, TargetKind::Picker);
    let names: Vec<&str> = inventory
        .targets
        .iter()
        .filter(|info| info.kind == TargetKind::App)
        .map(|info| info.name.as_str())
        .collect();
    assert_eq!(names, ["Fake One", "Fake Two"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app10_a_configured_target_whose_app_is_gone_is_missing() {
    let config = "[browsers]\nalternative = { app = \"gone.desktop\" }\n";
    let Some(service) = Service::start(config).await else {
        return;
    };
    let inventory = targets(&service).await;
    let gone = inventory
        .targets
        .iter()
        .find(|info| info.target == serde_json::json!({"app": "gone.desktop"}))
        .expect("listed");
    assert!(gone.missing);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dlg_app_lists_browsers_or_every_app() {
    let Some(service) = Service::start("").await else {
        return;
    };
    service
        .desktop
        .app("editor.desktop", "Editor", "editor %f", false);
    service.wye().await.rescan().await.expect("BRW-06");
    let decode = |text: String| -> AppList { json::decode("apps", &text).expect("AppList") };
    let wye = service.wye().await;
    let browsers = decode(wye.get_apps(false).await.expect("GetApps"));
    let ids: Vec<&str> = browsers.apps.iter().map(|app| app.id.as_str()).collect();
    assert_eq!(ids, [ONE, TWO]);
    let all = decode(wye.get_apps(true).await.expect("GetApps"));
    assert!(
        all.apps
            .iter()
            .any(|app| app.id == "editor.desktop" && !app.is_browser)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app03_services_and_the_expansion_catalogue_decode() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let services: ServiceList =
        json::decode("services", &wye.get_services().await.expect("GetServices"))
            .expect("ServiceList");
    assert!(!services.services.is_empty());
    assert!(
        services
            .services
            .iter()
            .all(|service| service.target == serde_json::json!({"default": true}))
    );
    let catalogue: ExpansionCatalogue = json::decode(
        "expansion",
        &wye.get_expansion_catalogue().await.expect("DLG-EXP"),
    )
    .expect("ExpansionCatalogue");
    assert!(!catalogue.wrappers.is_empty());
    assert!(
        catalogue
            .wrappers
            .iter()
            .all(|wrapper| !wrapper.hosts.is_empty())
    );
    assert!(catalogue.short_links.iter().all(|link| link.enabled));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn brw06_rescan_finds_a_new_browser_and_bumps_the_revision() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let before = wye.inventory_revision().await.expect("revision");
    wye.rescan().await.expect("nothing new");
    assert_eq!(wye.inventory_revision().await.expect("revision"), before);
    service
        .desktop
        .app("fake-three.desktop", "Fake Three", "fake-three %u", true);
    wye.rescan().await.expect("rescanned");
    eventually("InventoryRevision bumps", || async {
        wye.inventory_revision().await.is_ok_and(|now| now > before)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disc02_an_installed_app_is_noticed_without_a_rescan() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let before = wye.inventory_revision().await.expect("revision");
    // Give the watcher time to arm.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    service
        .desktop
        .app("fake-three.desktop", "Fake Three", "fake-three %u", true);
    eventually("the watcher rescans", || async {
        wye.inventory_revision().await.is_ok_and(|now| now > before)
    })
    .await;
    assert!(
        targets(&service)
            .await
            .targets
            .iter()
            .any(|info| info.name == "Fake Three")
    );
}
