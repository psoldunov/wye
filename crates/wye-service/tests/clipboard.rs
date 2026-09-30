//! The clipboard on a private bus with a fake clipboard (IN-02 to IN-04,
//! TRAY-10, EXT-02, EXT-03, EXT-05, EXT-12, EXT-13, EXT-15). Skips without
//! `dbus-daemon`.

mod support;

use support::{Service, eventually};
use wye_api::Error;
use wye_core::clipboard::songlink_api_url;
use wye_service::platform::HttpResponse;

const BROWSERS: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n\
                        alternative = { app = \"fake-two.desktop\" }\n";

fn argv(program: &str, url: &str) -> Vec<String> {
    vec![program.to_owned(), url.to_owned()]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tray10_clipboard_has_url_only_for_one_link() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let wye = service.wye().await;
    assert!(!wye.clipboard_has_url().await.expect("empty"));
    service.fakes.clipboard.copy("https://example.com/");
    assert!(wye.clipboard_has_url().await.expect("a link"));
    service.fakes.clipboard.copy("see https://example.com/");
    assert!(!wye.clipboard_has_url().await.expect("a sentence"));
    service.fakes.clipboard.copy("ftp://example.com/");
    assert!(!wye.clipboard_has_url().await.expect("not a web link"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in02_open_clipboard_routes_the_link_through_the_pipeline() {
    let Some(service) = Service::start(BROWSERS).await else {
        return;
    };
    service
        .fakes
        .clipboard
        .copy(" https://example.com/?utm_source=x \n");
    service
        .wye()
        .await
        .open_clipboard(false)
        .await
        .expect("opened");
    // PIPE-04 cleaned it on the way, like any link.
    assert_eq!(
        service.launched(),
        [argv("fake-one", "https://example.com/")]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in04_the_alternative_opens_in_the_alternative_browser() {
    let Some(service) = Service::start(BROWSERS).await else {
        return;
    };
    service.fakes.clipboard.copy("https://example.com/");
    service
        .wye()
        .await
        .open_clipboard(true)
        .await
        .expect("opened");
    assert_eq!(
        service.launched(),
        [argv("fake-two", "https://example.com/")]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in02_no_link_on_the_clipboard_is_not_found() {
    let Some(service) = Service::start(BROWSERS).await else {
        return;
    };
    service.fakes.clipboard.copy("just words");
    let refused = service.wye().await.open_clipboard(false).await;
    assert!(matches!(refused, Err(Error::NotFound(_))), "{refused:?}");
    assert!(service.launched().is_empty());
}

/// A service with `config` whose clipboard watcher is running.
async fn watching(config: &str) -> Option<Service> {
    let service = Service::start(config).await?;
    // Let the watcher subscribe.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    Some(service)
}

/// Copy `copied` and wait until exactly `expected` was written back.
async fn rewrites(service: &Service, copied: &str, expected: &str) {
    service.fakes.clipboard.copy(copied);
    eventually(expected, || async {
        service.fakes.clipboard.written() == [expected]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext02_a_copied_link_loses_its_tracking_once() {
    let Some(service) = watching("[extras]\nstrip-tracking-on-copy = true\n").await else {
        return;
    };
    rewrites(
        &service,
        "https://example.com/a?utm_source=x&b=1",
        "https://example.com/a?b=1",
    )
    .await;
    // The written text comes back as a change; it is Wye's own (EXT-12).
    service.fakes.clipboard.copy("https://example.com/a?b=1");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(service.fakes.clipboard.written().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext13_mailto_is_removed_with_its_query() {
    let Some(service) = watching("[extras]\nstrip-mailto-on-copy = true\n").await else {
        return;
    };
    rewrites(
        &service,
        "mailto:name@example.com?subject=Hi",
        "name@example.com",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext15_a_music_link_becomes_its_songlink_page() {
    let Some(service) = watching("[extras]\nsonglink-on-copy = true\n").await else {
        return;
    };
    let track = "https://open.spotify.com/track/abc";
    service.fakes.http.respond(
        songlink_api_url(&url::Url::parse(track).expect("valid")).as_str(),
        HttpResponse {
            status: 200,
            location: None,
            body: Some(r#"{"pageUrl": "https://song.link/s/abc"}"#.to_owned()),
        },
    );
    rewrites(&service, track, "https://song.link/s/abc").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext15_without_an_answer_the_clipboard_stays() {
    let Some(service) = watching("[extras]\nsonglink-on-copy = true\n").await else {
        return;
    };
    service
        .fakes
        .clipboard
        .copy("https://open.spotify.com/track/abc");
    eventually("Songlink was asked", || async {
        !service.fakes.http.requests().is_empty()
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(service.fakes.clipboard.written().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext12_nothing_is_rewritten_while_every_switch_is_off() {
    let Some(service) = watching("").await else {
        return;
    };
    service
        .fakes
        .clipboard
        .copy("https://example.com/?utm_source=x");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(service.fakes.clipboard.written().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_the_clipboard_mechanisms() {
    let Some(service) = Service::start("").await else {
        return;
    };
    let capabilities = service.status().await.capabilities;
    assert_eq!(capabilities.clipboard_read.as_deref(), Some("fake"));
    assert_eq!(capabilities.clipboard_watch.as_deref(), Some("fake"));
}
