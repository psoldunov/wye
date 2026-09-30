//! Network expansion in `TestLink` on a private bus with a fake HTTP
//! client (PIPE-03, IN-08, DLG-EXP-03): short links are followed unless the
//! tester passes `skip-network`. Skips without `dbus-daemon`.

mod support;

use std::collections::HashMap;

use support::Service;
use wye_api::trace::LinkTrace;
use wye_api::{context, json};
use wye_service::platform::HttpResponse;
use zbus::zvariant::Value;

const PRIMARY_ONE: &str = "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n";
const SHORT: &str = "https://bit.ly/abc";

async fn trace(service: &Service, skip_network: bool) -> LinkTrace {
    let link = HashMap::from([
        (context::ENTRY, Value::from("cli")),
        (context::SKIP_NETWORK, Value::from(skip_network)),
    ]);
    let text = service
        .wye()
        .await
        .test_link(SHORT, link)
        .await
        .expect("traced");
    json::decode("trace", &text).expect("LinkTrace")
}

fn redirect_to_example(service: &Service) {
    service.fakes.http.respond(
        SHORT,
        HttpResponse {
            status: 301,
            location: Some("https://example.com/landing".to_owned()),
            body: None,
        },
    );
}

/// Trace the short link that redirects to example.com, with or without the
/// network; also returns how many requests were sent.
async fn traced_redirect(skip_network: bool) -> Option<(LinkTrace, usize)> {
    let service = Service::start(PRIMARY_ONE).await?;
    redirect_to_example(&service);
    let traced = trace(&service, skip_network).await;
    Some((traced, service.fakes.http.requests().len()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe03_test_link_follows_a_short_link() {
    let Some((traced, sent)) = traced_redirect(false).await else {
        return;
    };
    assert_eq!(
        traced.final_url.as_deref(),
        Some("https://example.com/landing")
    );
    assert!(traced.steps.iter().any(|step| step.kind == "expand"));
    assert_eq!(sent, 1, "only the enabled short-link domain is asked");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in08_skip_network_contacts_nobody() {
    let Some((traced, sent)) = traced_redirect(true).await else {
        return;
    };
    assert_eq!(traced.final_url.as_deref(), Some(SHORT));
    assert_eq!(sent, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe03_a_failing_service_leaves_the_link_as_it_is() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    // No fake answer: the request fails.
    let traced = trace(&service, false).await;
    assert_eq!(traced.final_url.as_deref(), Some(SHORT));
    assert!(traced.steps.iter().any(|step| step.kind == "expand"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe03_an_opened_short_link_launches_its_destination() {
    let Some(service) = Service::start(PRIMARY_ONE).await else {
        return;
    };
    redirect_to_example(&service);
    let link = HashMap::from([(context::ENTRY, Value::from("cli"))]);
    service
        .wye()
        .await
        .open_link(SHORT, link)
        .await
        .expect("opened");
    assert_eq!(
        service.launched(),
        [vec![
            "fake-one".to_owned(),
            "https://example.com/landing".to_owned()
        ]]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dlg_exp04_a_link_that_cannot_be_expanded_is_announced_when_asked() {
    let config = format!("{PRIMARY_ONE}[advanced.expansion]\nnotify-on-failure = true\n");
    let Some(service) = Service::start(&config).await else {
        return;
    };
    let link = HashMap::from([(context::ENTRY, Value::from("cli"))]);
    service
        .wye()
        .await
        .open_link(SHORT, link)
        .await
        .expect("opened unexpanded");
    assert_eq!(
        service.launched(),
        [vec!["fake-one".to_owned(), SHORT.to_owned()]]
    );
    support::eventually("the notification", || async {
        service
            .fakes
            .notifier
            .shown()
            .iter()
            .any(|shown| shown.summary.contains("short link"))
    })
    .await;
}
