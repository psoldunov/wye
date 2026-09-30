//! The configuration on a private bus (SET-06, TRAY-11, KEY-04, GEN-01,
//! risk 16): `GetConfig`, `UpdateConfig` and its errors, `SetPrimary`,
//! `GetDefaults`, reload on file change, and the autostart entry. Skips
//! without `dbus-daemon`.

mod support;

use serde_json::Value;
use support::{ONE, Service, eventually};
use wye_api::Error;

const CONFIG: &str = "config/wye/config.toml";
const AUTOSTART: &str = "config/autostart/dev.soldunov.wye.desktop";

async fn config_json(service: &Service) -> (Value, u64) {
    let (text, revision) = service.wye().await.get_config().await.expect("GetConfig");
    (serde_json::from_str(&text).expect("JSON"), revision)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_config_returns_the_file_in_its_own_shape() {
    let Some(service) = Service::start("[general]\nlaunch-at-login = false\n").await else {
        return;
    };
    let (config, revision) = config_json(&service).await;
    assert_eq!(config["general"]["launch-at-login"], Value::Bool(false));
    assert!(revision >= 1, "0 is reserved for 'skip the check'");
    assert_eq!(
        service
            .wye()
            .await
            .config_revision()
            .await
            .expect("property"),
        revision
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_update_config_saves_and_bumps_the_revision() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let (_, before) = config_json(&service).await;
    let after = wye
        .update_config(r#"{"extras": {"force-https": true}}"#, before)
        .await
        .expect("saved");
    assert_eq!(after, before + 1);
    assert!(service.desktop.read(CONFIG).contains("force-https = true"));
    let (config, revision) = config_json(&service).await;
    assert_eq!(config["extras"]["force-https"], Value::Bool(true));
    assert_eq!(revision, after);

    // The same change again writes nothing and keeps the revision.
    assert_eq!(
        wye.update_config(r#"{"extras": {"force-https": true}}"#, 0)
            .await
            .expect("no-op"),
        after
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_a_stale_revision_is_a_conflict() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let (_, revision) = config_json(&service).await;
    wye.update_config(r#"{"extras": {"force-https": true}}"#, revision)
        .await
        .expect("saved");
    let stale = wye
        .update_config(r#"{"extras": {"force-https": false}}"#, revision)
        .await;
    assert!(matches!(stale, Err(Error::Conflict(_))), "{stale:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_a_symlinked_file_is_read_only() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let store = service.desktop.path("store-config.toml");
    std::fs::write(&store, "[extras]\nforce-https = true\n").expect("written");
    let config = service.desktop.path(CONFIG);
    std::fs::remove_file(&config).expect("removed");
    std::os::unix::fs::symlink(&store, &config).expect("linked");
    // A new environment makes the service read the file again.
    service.ctx.set_environment(service.desktop.environment());

    let refused = service
        .wye()
        .await
        .update_config(r#"{"extras": {"force-https": false}}"#, 0)
        .await;
    assert!(matches!(refused, Err(Error::ReadOnly(_))), "{refused:?}");
    let status = service.status().await;
    assert!(!status.config.writable);
    assert_eq!(
        std::fs::read_to_string(&store).expect("read"),
        "[extras]\nforce-https = true\n",
        "the linked file is untouched"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_a_file_with_unknown_keys_is_not_lossless() {
    let Some(service) = Service::start("surprise = 1\n").await else {
        return;
    };
    let refused = service
        .wye()
        .await
        .update_config(r#"{"extras": {"force-https": true}}"#, 0)
        .await;
    assert!(matches!(refused, Err(Error::NotLossless(_))), "{refused:?}");
    assert_eq!(service.desktop.read(CONFIG), "surprise = 1\n");
    assert!(!service.status().await.config.lossless);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_an_invalid_patch_lists_its_problems() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let wrong_type = wye
        .update_config(r#"{"general": {"launch-at-login": "yes"}}"#, 0)
        .await;
    assert!(
        matches!(wrong_type, Err(Error::InvalidArgs(_))),
        "{wrong_type:?}"
    );
    let not_json = wye.update_config("{", 0).await;
    assert!(
        matches!(not_json, Err(Error::InvalidArgs(_))),
        "{not_json:?}"
    );
    let default_primary = wye.set_primary(r#"{"default": true}"#).await;
    let Err(Error::InvalidArgs(message)) = default_primary else {
        panic!("accepted Default as the primary browser: {default_primary:?}");
    };
    assert!(message.contains("browsers.primary"), "{message}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tray11_set_primary_replaces_the_target() {
    let Some(service) = Service::start("").await else {
        return;
    };
    service
        .wye()
        .await
        .set_primary(&format!(r#"{{"app": "{ONE}"}}"#))
        .await
        .expect("set");
    let (config, _) = config_json(&service).await;
    assert_eq!(
        config["browsers"]["primary"],
        serde_json::json!({"app": ONE})
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key04_get_defaults_returns_one_section() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    let keys: Value =
        serde_json::from_str(&wye.get_defaults("picker.keys").await.expect("defaults"))
            .expect("JSON");
    assert!(keys.get("open").is_some(), "{keys}");
    let unknown = wye.get_defaults("nowhere").await;
    assert!(matches!(unknown, Err(Error::InvalidArgs(_))), "{unknown:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set06_an_edited_file_is_reloaded() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let (_, before) = config_json(&service).await;
    service
        .desktop
        .replace(CONFIG, "[extras]\nforce-https = true\n");
    eventually("the edited file is reloaded", || async {
        let (config, revision) = config_json(&service).await;
        revision > before && config["extras"]["force-https"] == Value::Bool(true)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn risk16_a_broken_file_keeps_the_last_good_configuration() {
    let Some(service) = Service::start("[extras]\nforce-https = true\n").await else {
        return;
    };
    let (_, before) = config_json(&service).await;
    service.desktop.replace(CONFIG, "[extras\n");
    eventually("the error is reported", || async {
        service.status().await.config.error.is_some()
    })
    .await;
    let (config, revision) = config_json(&service).await;
    assert!(revision > before);
    assert_eq!(config["extras"]["force-https"], Value::Bool(true));
    let shown = service.fakes.notifier.shown();
    assert_eq!(shown.len(), 1, "one notification: {shown:?}");
    assert!(shown[0].summary.contains("configuration"));
    // Saving now would overwrite what the user is editing.
    let refused = service.wye().await.update_config("{}", 0).await;
    assert!(matches!(refused, Err(Error::NotLossless(_))), "{refused:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gen01_launch_at_login_follows_the_setting() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye_path = service.desktop.path("bin/wye");
    service.ctx.set_wye_executable(wye_path.clone());
    let wye = service.wye().await;
    wye.update_config(r#"{"general": {"launch-at-login": true}}"#, 0)
        .await
        .expect("saved");
    let entry = service.desktop.read(AUTOSTART);
    assert!(
        entry.contains(&format!("Exec={} service --activate", wye_path.display())),
        "{entry}"
    );
    wye.update_config(r#"{"general": {"launch-at-login": false}}"#, 0)
        .await
        .expect("saved");
    assert!(!service.desktop.path(AUTOSTART).exists());
}
