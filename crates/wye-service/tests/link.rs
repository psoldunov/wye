//! The link path on a private bus (IN-01, IN-07, PIPE-01, PIPE-02, PIPE-15,
//! LAUNCH-03, LAUNCH-06, LAUNCH-07, PKS-05, PKS-07): links arrive through
//! `OpenLink` and `org.freedesktop.Application.Open`, and the fake launcher
//! records what would have started. Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;

use support::{ONE, Service, TWO, eventually};
use wye_api::proxy::{ApplicationProxy, Wye1Proxy};
use wye_api::{Error, context};
use zbus::zvariant::Value;

const URL: &str = "https://example.com/";
const PRIMARY_ONE: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";

fn cli() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([(context::ENTRY, Value::from("cli"))])
}

async fn wye(service: &Service) -> Wye1Proxy<'static> {
    Wye1Proxy::new(&service.client).await.expect("proxy")
}

fn argv(program: &str, url: &str) -> Vec<String> {
    vec![program.to_owned(), url.to_owned()]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_link_launches_the_primary_browser_in_its_own_scope() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    wye(&service)
        .await
        .open_link("https://example.com/?utm_source=x", cli())
        .await
        .expect("opened");
    // PIPE-04 cleaned the link on the way.
    assert_eq!(service.launched(), [argv("fake-one", URL)]);
    let adopted = service.fakes.scope.adopted();
    assert_eq!(adopted.len(), 1, "LAUNCH-06: the app gets a scope");
    assert_eq!(adopted[0].1.desktop_id.as_deref(), Some(ONE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_activation_token_reaches_the_app() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    let mut link = cli();
    link.insert(context::ACTIVATION_TOKEN, Value::from("token-1"));
    wye(&service)
        .await
        .open_link(URL, link)
        .await
        .expect("opened");

    let application = ApplicationProxy::new(&service.client).await.expect("proxy");
    let platform_data = HashMap::from([
        (context::PLATFORM_ACTIVATION_TOKEN, Value::from("token-2")),
        (context::PLATFORM_STARTUP_ID, Value::from("startup-2")),
    ]);
    application
        .open(&[URL], platform_data)
        .await
        .expect("opened");

    let launched = service.fakes.launcher.launched();
    let env = |index: usize, name: &str| {
        launched[index]
            .env
            .iter()
            .find(|(key, _)| key == name)
            .and_then(|(_, value)| value.clone())
    };
    assert_eq!(env(0, "XDG_ACTIVATION_TOKEN").as_deref(), Some("token-1"));
    assert_eq!(env(0, "DESKTOP_STARTUP_ID"), None);
    assert_eq!(env(1, "XDG_ACTIVATION_TOKEN").as_deref(), Some("token-2"));
    assert_eq!(env(1, "DESKTOP_STARTUP_ID").as_deref(), Some("startup-2"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_detects_the_source_from_the_callers_pid() {
    let rule = format!(
        "{PRIMARY_ONE}\n[[rules]]\nname = \"From chat\"\ntarget = {{ app = \"{TWO}\" }}\n\
         url-matchers = [{{ kind = \"domain\", pattern = \"example.com\" }}]\n\
         source-apps = [\"org.example.Chat.desktop\"]\n"
    );
    let Some(service) = Service::start(&rule).await else {
        return;
    };
    // The client connection belongs to this process: make it the chat app.
    service.desktop.process(
        std::process::id(),
        "chat",
        1,
        "/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.example.Chat-1.scope",
    );
    let application = ApplicationProxy::new(&service.client).await.expect("proxy");
    application
        .open(&[URL, "https://example.com/two"], HashMap::new())
        .await
        .expect("opened");
    assert_eq!(
        service.launched(),
        [
            argv("fake-two", URL),
            argv("fake-two", "https://example.com/two")
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_named_source_app_is_used_as_given() {
    let rule = format!(
        "{PRIMARY_ONE}\n[[rules]]\nname = \"From chat\"\ntarget = {{ app = \"{TWO}\" }}\n\
         url-matchers = [{{ kind = \"domain\", pattern = \"example.com\" }}]\n\
         source-apps = [\"org.example.Chat.desktop\"]\n"
    );
    let Some(service) = Service::start(&rule).await else {
        return;
    };
    let mut link = cli();
    link.insert(
        context::SOURCE_DESKTOP_ID,
        Value::from("org.example.Chat.desktop"),
    );
    wye(&service)
        .await
        .open_link(URL, link)
        .await
        .expect("opened");
    assert_eq!(service.launched(), [argv("fake-two", URL)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_link_is_refused_and_notified() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    let error = wye(&service)
        .await
        .open_link("ftp://example.com/file", cli())
        .await
        .expect_err("PIPE-02: rejected");
    assert!(matches!(error, Error::InvalidArgs(_)), "{error}");
    let shown = service.fakes.notifier.shown();
    assert_eq!(shown.len(), 1);
    assert!(shown[0].summary.contains("can't open"), "{shown:?}");
    assert!(
        shown[0].body.contains("ftp://example.com/file"),
        "{shown:?}"
    );
    assert!(service.launched().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_context_is_invalid_args() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    let link = HashMap::from([(context::FORCE, Value::from("sideways"))]);
    let error = wye(&service)
        .await
        .open_link(URL, link)
        .await
        .expect_err("refused");
    assert!(matches!(error, Error::InvalidArgs(_)), "{error}");
    assert!(service.fakes.notifier.shown().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_launch_offers_other_browsers() {
    let config = format!(
        "[browsers]\nprimary = {{ app = \"term.desktop\" }}\n\n\
         [[browsers.shown]]\ntarget = {{ app = \"{TWO}\" }}\n"
    );
    let Some(service) = Service::start(&config).await else {
        return;
    };
    service.desktop.write(
        "data/applications/term.desktop",
        "[Desktop Entry]\nType=Application\nName=Term Browser\nExec=term %u\nTerminal=true\n\
         MimeType=x-scheme-handler/https;\n",
    );
    // The service keeps its inventory; a new entry needs a rescan.
    wye(&service).await.rescan().await.expect("rescanned");
    let error = wye(&service)
        .await
        .open_link(URL, cli())
        .await
        .expect_err("LAUNCH-07: the launch fails");
    assert!(matches!(error, Error::Failed(_)), "{error}");
    let shown = service.fakes.notifier.shown();
    assert_eq!(shown.len(), 1);
    assert!(shown[0].summary.contains("Term Browser"), "{shown:?}");
    assert_eq!(
        shown[0].actions,
        [("open:0".to_owned(), "Open in Fake Two".to_owned())]
    );

    service.fakes.notifier.press(1, "open:0");
    eventually("the alternative to open", || async {
        service.launched() == [argv("fake-two", URL)]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_picker_stand_in_opens_the_previous_default() {
    // The state is written before the service starts: the first scan saves
    // the browsers it saw to the same file (SHOWN-09) and could otherwise
    // overwrite a file written behind its back.
    let Some(service) =
        Service::start_with("[browsers]\nprimary = { picker = true }\n", |desktop| {
            desktop.write(
                "state/wye/state.toml",
                "previous-default-browser = \"fake-two.desktop\"\n",
            );
        })
        .await
    else {
        return;
    };
    wye(&service)
        .await
        .open_link(URL, cli())
        .await
        .expect("decided");
    // `OpenLink` answers once the link is decided; the stand-in follows
    // when the UI host turns out to be missing (PIPE-13).
    eventually("the stand-in to open", || async {
        service.launched() == [argv("fake-two", URL)]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_locked_screen_opens_the_alternative() {
    // PKS-05: with the picker skipped while locked, the alternative opens.
    let config = format!(
        "[browsers]\nprimary = {{ picker = true }}\nalternative = {{ app = \"{TWO}\" }}\n\n\
         [picker]\nskip-when-locked = true\n"
    );
    let Some(service) = Service::start(&config).await else {
        return;
    };
    service.fakes.lock.set(true);
    wye(&service)
        .await
        .open_link(URL, cli())
        .await
        .expect("opened");
    assert_eq!(service.launched(), [argv("fake-two", URL)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_link_opens_once_the_screen_unlocks() {
    // PKS-07: the alternative is the picker too, so the link waits.
    let config = "[browsers]\nprimary = { picker = true }\n\n[picker]\nskip-when-locked = true\n";
    let Some(service) = Service::start(config).await else {
        return;
    };
    service.fakes.lock.set(true);
    let proxy = wye(&service).await;
    proxy
        .open_link("https://example.com/old", cli())
        .await
        .expect("held");
    proxy.open_link(URL, cli()).await.expect("held");
    assert!(service.launched().is_empty(), "nothing opens while locked");

    service.fakes.lock.set(false);
    eventually("the held link to open", || async {
        !service.launched().is_empty()
    })
    .await;
    // A newer link replaces the held one; "Fake One" sorts first.
    assert_eq!(service.launched(), [argv("fake-one", URL)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activate_without_a_ui_host_still_answers() {
    // TRAY-05: Settings is asked for; the missing UI host is only logged.
    let Some(service) = Service::start("").await else {
        return;
    };
    let application = ApplicationProxy::new(&service.client).await.expect("proxy");
    application
        .activate(HashMap::new())
        .await
        .expect("answered");
    let error = wye(&service)
        .await
        .show_window("settings", "")
        .await
        .expect_err("no UI host on the private bus");
    assert!(matches!(error, Error::Unavailable(_)), "{error}");
}

/// ADV-11: `advanced.held-keys = "off"` from the configuration in use: the
/// probe is not asked, a held Shift picks nothing, and `Status` says held
/// keys are unavailable (KEY-06).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_keys_off_skips_the_probe() {
    let config = format!(
        "[browsers]\nprimary = {{ app = \"{ONE}\" }}\nalternative = {{ app = \"{TWO}\" }}\n\n\
         [advanced]\nheld-keys = \"off\"\n"
    );
    let Some(service) = Service::start(&config).await else {
        return;
    };
    service
        .fakes
        .modifiers
        .set(Some(vec![context::Modifier::Shift]));
    wye(&service)
        .await
        .open_link(URL, cli())
        .await
        .expect("opened");
    assert_eq!(service.launched(), [argv("fake-one", URL)]);
    assert_eq!(service.status().await.capabilities.held_keys, None);
}
