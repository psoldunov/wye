//! HTTP for short-link expansion and Songlink (PIPE-03, EXT-15).
//!
//! Planned: ureq 3 with rustls and the platform verifier, redirects off, no
//! cookies (decision 7). Until then no request is sent and links stay
//! unexpanded.

use super::{HttpClient, HttpRequest, HttpResponse, PlatformError};

/// The network is never contacted.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHttp;

impl HttpClient for NoHttp {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, PlatformError> {
        Err(PlatformError::Unavailable(
            "HTTP is not implemented yet".to_owned(),
        ))
    }
}
