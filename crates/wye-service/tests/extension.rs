//! The service writes the browser extension's host manifests when it starts
//! and when a browser's directory appears (BEXT-04), into its own
//! environment only, unless the user opted out with `wye extension remove`.
//! Skips without `dbus-daemon`.

mod support;

use std::time::Duration;

use support::{Desktop, Service, eventually};

const FIREFOX_MANIFEST: &str = "home/.mozilla/native-messaging-hosts/dev.soldunov.wye.json";
const CHROMIUM_MANIFEST: &str = "config/chromium/NativeMessagingHosts/dev.soldunov.wye.json";
const STATE: &str = "state/wye/state.toml";

/// Firefox has run, and `wye-native-host` is on the search path.
fn firefox_and_host(desktop: &Desktop) {
    desktop.write("home/.mozilla/firefox/profiles.ini", "");
    desktop.write("bin/wye-native-host", "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_service_writes_the_manifests_at_start_bext_04() {
    let Some(service) = Service::start_with("", firefox_and_host).await else {
        return;
    };
    let manifest = service.desktop.path(FIREFOX_MANIFEST);
    eventually("the Firefox manifest", || {
        let manifest = manifest.clone();
        async move { manifest.is_file() }
    })
    .await;
    let text = service.desktop.read(FIREFOX_MANIFEST);
    let host = service.desktop.path("bin/wye-native-host");
    assert!(text.contains(&host.display().to_string()), "{text}");
    assert!(text.contains("wye@soldunov.dev"), "{text}");
}

/// A browser installed and first run while the service runs gets its
/// manifest without a restart: the watcher sees its directory appear.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_browser_appearing_later_gets_its_manifest_bext_04() {
    let Some(service) = Service::start_with("", |desktop| {
        desktop.write("bin/wye-native-host", "");
    })
    .await
    else {
        return;
    };
    // Let the watcher arm, so the directories below appear under it.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!service.desktop.path(CHROMIUM_MANIFEST).exists());

    std::fs::create_dir_all(service.desktop.path("config/chromium/Default")).expect("chromium");
    std::fs::create_dir_all(service.desktop.path("home/.mozilla/firefox")).expect("firefox");

    for relative in [CHROMIUM_MANIFEST, FIREFOX_MANIFEST] {
        let manifest = service.desktop.path(relative);
        eventually(relative, || {
            let manifest = manifest.clone();
            async move { manifest.is_file() }
        })
        .await;
    }
    let text = service.desktop.read(CHROMIUM_MANIFEST);
    assert!(text.contains("chrome-extension://"), "{text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_opt_out_keeps_the_manifests_away_bext_04() {
    let Some(service) = Service::start_with("", |desktop| {
        firefox_and_host(desktop);
        desktop.write(STATE, "extension-host-removed = true\n");
    })
    .await
    else {
        return;
    };
    // Nothing to wait for when nothing is written: give the startup task
    // ample time, then check.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!service.desktop.path(FIREFOX_MANIFEST).exists());
    assert!(
        !service
            .desktop
            .path("home/.mozilla/native-messaging-hosts")
            .exists()
    );
    assert_eq!(
        service.desktop.read(STATE),
        "extension-host-removed = true\n"
    );
}
