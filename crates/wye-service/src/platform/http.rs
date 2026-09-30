//! HTTP for short-link expansion and Songlink (PIPE-03, DLG-EXP-03, EXT-15).
//!
//! ureq 3 with rustls and the platform verifier (decision 7): the system's
//! trust store, no OpenSSL. Every request is one hop: redirects are never
//! followed (the core owns the redirect loop), there is no cookie store, and
//! a `HEAD` or body-less `GET` never reads the body. Each request carries its
//! own whole-request timeout. Blocking: callers run it on a blocking thread.
//!
//! [`Resolver`] adapts a client to the core's `ShortLinkResolver` hook.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ureq::http::StatusCode;
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;
use wye_core::Hooks;
use wye_core::hooks::{Location, ResolveError, ShortLinkResolver};

use super::{HttpClient, HttpMethod, HttpRequest, HttpResponse, PlatformError};

/// The largest body read (Songlink answers are a few kilobytes).
const MAX_BODY: u64 = 1024 * 1024;

/// `User-Agent` sent with every request.
const USER_AGENT: &str = concat!("Wye/", env!("CARGO_PKG_VERSION"));

/// The network is never contacted.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHttp;

impl HttpClient for NoHttp {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, PlatformError> {
        Err(PlatformError::Unavailable(
            "network access is not available".to_owned(),
        ))
    }
}

/// Requests over ureq.
#[derive(Debug)]
pub struct UreqClient {
    agent: ureq::Agent,
}

impl Default for UreqClient {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqClient {
    /// A client that trusts the system's certificate store.
    #[must_use]
    pub fn new() -> Self {
        Self::with_tls(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
    }

    fn with_tls(tls: TlsConfig) -> Self {
        let config = ureq::Agent::config_builder()
            // DLG-EXP-03: one hop per request; the core follows redirects.
            .max_redirects(0)
            // A 3xx or 4xx is an answer, not an error.
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .tls_config(tls)
            .build();
        Self {
            agent: config.new_agent(),
        }
    }
}

impl HttpClient for UreqClient {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, PlatformError> {
        let timeout = Some(request.timeout);
        let sent = match request.method {
            HttpMethod::Head => self
                .agent
                .head(&request.url)
                .config()
                .timeout_global(timeout)
                .build()
                .call(),
            HttpMethod::Get => self
                .agent
                .get(&request.url)
                .config()
                .timeout_global(timeout)
                .build()
                .call(),
        };
        let mut response = sent.map_err(|error| to_platform(error, request.timeout))?;
        let status = response.status();
        let location = is_redirect(status)
            .then(|| response.headers().get("location"))
            .flatten()
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = if request.read_body {
            let text = response
                .body_mut()
                .with_config()
                .limit(MAX_BODY)
                .read_to_string()
                .map_err(|error| to_platform(error, request.timeout))?;
            Some(text)
        } else {
            None
        };
        Ok(HttpResponse {
            status: status.as_u16(),
            location,
            body,
        })
    }
}

fn is_redirect(status: StatusCode) -> bool {
    status.is_redirection()
}

fn to_platform(error: ureq::Error, timeout: Duration) -> PlatformError {
    match error {
        ureq::Error::Timeout(_) => PlatformError::Timeout(timeout),
        other => PlatformError::Failed(other.to_string()),
    }
}

/// Answers that mean "this server does not do `HEAD`": ask again with `GET`.
const HEAD_REFUSED: [u16; 3] = [403, 405, 501];

/// One link's resolver: every hop shares the deadline set when it was made.
pub struct Resolver {
    http: Arc<dyn HttpClient>,
    deadline: Instant,
}

impl std::fmt::Debug for Resolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Resolver")
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

impl Resolver {
    /// A resolver over `http` that gives up `timeout` from now.
    #[must_use]
    pub fn new(http: Arc<dyn HttpClient>, timeout: Duration) -> Self {
        Self {
            http,
            deadline: Instant::now() + timeout,
        }
    }

    /// The pipeline hooks with this resolver (PIPE-03).
    #[must_use]
    pub fn hooks(&self) -> Hooks<'_> {
        Hooks::none().with_short_links(self)
    }

    fn request(&self, method: HttpMethod, url: &Url) -> Result<HttpResponse, ResolveError> {
        let timeout = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or(ResolveError::Timeout)?;
        let request = HttpRequest {
            method,
            url: url.to_string(),
            timeout,
            read_body: false,
        };
        self.http.send(&request).map_err(|error| match error {
            PlatformError::Timeout(_) => ResolveError::Timeout,
            other => ResolveError::Failed(other.to_string()),
        })
    }
}

impl ShortLinkResolver for Resolver {
    fn resolve(&self, url: &Url) -> Result<Option<Location>, ResolveError> {
        let head = self.request(HttpMethod::Head, url)?;
        let answer = if HEAD_REFUSED.contains(&head.status) {
            self.request(HttpMethod::Get, url)?
        } else {
            head
        };
        Ok(answer.location.map(Location::new))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::fake::FakeHttp;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("valid")
    }

    #[test]
    fn a_redirect_names_the_next_hop() {
        let http = Arc::new(FakeHttp::default());
        http.respond(
            "https://bit.ly/x",
            HttpResponse {
                status: 301,
                location: Some("https://example.com/".to_owned()),
                body: None,
            },
        );
        let resolver = Resolver::new(http.clone(), Duration::from_secs(1));
        let next = resolver
            .resolve(&url("https://bit.ly/x"))
            .expect("answered");
        assert_eq!(next, Some(Location::new("https://example.com/")));
        let sent = http.requests();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].method, HttpMethod::Head, "DLG-EXP-03: HEAD first");
        assert!(!sent[0].read_body);
    }

    #[test]
    fn a_spent_deadline_is_a_timeout_without_a_request() {
        let http = Arc::new(FakeHttp::default());
        let resolver = Resolver::new(http.clone(), Duration::ZERO);
        assert_eq!(
            resolver.resolve(&url("https://bit.ly/x")),
            Err(ResolveError::Timeout)
        );
        assert!(http.requests().is_empty());
    }
}
