//! Short-link expansion over real HTTP against a local stub server
//! (PIPE-03, DLG-EXP-03, DLG-EXP-04): `HEAD` first, `GET` when `HEAD` is
//! refused, no cookies, redirects not followed by the client, at most the
//! configured number of hops, and a deadline.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use url::Url;
use wye_core::config::ExpansionSettings;
use wye_core::expand::{ExpandStop, ExpansionCatalogue};
use wye_core::hooks::{ResolveError, ShortLinkResolver as _};
use wye_service::platform::http::{Resolver, UreqClient};

/// One request as the stub saw it.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<String>,
}

/// What the stub answers for a method and path: status and `Location`.
type Answer = fn(&str, &str) -> Option<(u16, Option<String>)>;

/// A local HTTP server answering with `answer`; `None` means "never reply".
struct Stub {
    address: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Stub {
    fn start(answer: Answer) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
        let address = listener.local_addr().expect("address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                std::thread::spawn(move || serve(stream, answer, &log));
            }
        });
        Self { address, seen }
    }

    fn url(&self, path: &str) -> Url {
        Url::parse(&format!("http://{}{path}", self.address)).expect("valid")
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("log").clone()
    }
}

fn serve(stream: TcpStream, answer: Answer, log: &Mutex<Vec<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        headers.push(line.trim().to_lowercase());
    }
    log.lock().expect("log").push(Seen {
        method: method.clone(),
        path: path.clone(),
        headers,
    });
    let Some((status, location)) = answer(&method, &path) else {
        // Hold the connection open without answering.
        std::thread::sleep(Duration::from_secs(5));
        return;
    };
    let location = location.map_or_else(String::new, |to| format!("Location: {to}\r\n"));
    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\n{location}Set-Cookie: tracker=1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

fn resolver(timeout: Duration) -> Resolver {
    Resolver::new(Arc::new(UreqClient::new()), timeout)
}

#[test]
fn dlg_exp03_head_answers_with_the_location_without_following_it() {
    let stub = Stub::start(|_, path| match path {
        "/short" => Some((301, Some("/long".to_owned()))),
        _ => Some((200, None)),
    });
    let next = resolver(Duration::from_secs(2))
        .resolve(&stub.url("/short"))
        .expect("answered");
    assert_eq!(
        next.map(|location| location.as_str().to_owned()).as_deref(),
        Some("/long")
    );
    let seen = stub.seen();
    assert_eq!(
        seen.len(),
        1,
        "the client does not follow redirects: {seen:?}"
    );
    assert_eq!(seen[0].method, "HEAD");
    assert_eq!(seen[0].path, "/short");
}

#[test]
fn dlg_exp03_a_refused_head_falls_back_to_get() {
    let stub = Stub::start(|method, _| match method {
        "HEAD" => Some((405, None)),
        _ => Some((302, Some("https://example.com/".to_owned()))),
    });
    let next = resolver(Duration::from_secs(2))
        .resolve(&stub.url("/short"))
        .expect("answered");
    assert!(next.is_some());
    let methods: Vec<String> = stub.seen().into_iter().map(|seen| seen.method).collect();
    assert_eq!(methods, ["HEAD", "GET"]);
}

#[test]
fn dlg_exp03_no_cookies_are_sent_back() {
    let stub = Stub::start(|_, path| match path {
        "/a" => Some((301, Some("/b".to_owned()))),
        "/b" => Some((301, Some("https://example.com/".to_owned()))),
        _ => Some((200, None)),
    });
    let resolver = resolver(Duration::from_secs(2));
    resolver.resolve(&stub.url("/a")).expect("answered");
    resolver.resolve(&stub.url("/b")).expect("answered");
    for seen in stub.seen() {
        assert!(
            !seen
                .headers
                .iter()
                .any(|header| header.starts_with("cookie:")),
            "{seen:?}"
        );
    }
}

#[test]
fn dlg_exp04_the_chain_stops_at_the_configured_number_of_redirects() {
    let stub = Stub::start(|_, path| {
        let n: u32 = path.trim_start_matches("/r").parse().unwrap_or(0);
        Some((301, Some(format!("/r{}", n + 1))))
    });
    let settings = ExpansionSettings {
        custom_short_links: vec!["127.0.0.1".to_owned()],
        max_redirects: 2,
        ..ExpansionSettings::default()
    };
    let expanded = ExpansionCatalogue::shipped().expand_short_link(
        &stub.url("/r0"),
        &settings,
        &resolver(Duration::from_secs(2)),
    );
    assert_eq!(expanded.stop, ExpandStop::LimitReached);
    assert_eq!(expanded.hops.len(), 2);
    assert_eq!(stub.seen().len(), 2);
}

#[test]
fn pipe03_a_silent_server_times_out_within_the_deadline() {
    let stub = Stub::start(|_, _| None);
    let started = Instant::now();
    let answer = resolver(Duration::from_millis(300)).resolve(&stub.url("/slow"));
    assert_eq!(answer, Err(ResolveError::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}
