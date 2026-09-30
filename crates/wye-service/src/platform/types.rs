//! Data the platform traits exchange.

use std::time::Duration;

/// Why a platform integration could not do something.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// The session has no way to do this (no portal, no protocol, no X11).
    #[error("not available in this session: {0}")]
    Unavailable(String),
    /// It was tried and failed.
    #[error("{0}")]
    Failed(String),
    /// No answer in time.
    #[error("timed out after {0:?}")]
    Timeout(Duration),
}

impl From<PlatformError> for wye_api::Error {
    fn from(error: PlatformError) -> Self {
        match error {
            PlatformError::Unavailable(message) => Self::Unavailable(message),
            other => Self::failed(other.to_string()),
        }
    }
}

/// The app whose window has keyboard focus (source-app step 4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusedApp {
    /// Process ID of the window's client.
    pub pid: Option<u32>,
    /// Desktop ID the compositor or window reports.
    pub desktop_id: Option<String>,
    /// X11 `WM_CLASS` or Wayland app ID.
    pub resource_class: Option<String>,
}

/// A desktop notification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notification {
    /// Title.
    pub summary: String,
    /// Text.
    pub body: String,
    /// Icon theme name or path; Wye's own icon when absent.
    pub icon: Option<String>,
    /// Buttons: action key and label.
    pub actions: Vec<(String, String)>,
    /// Replaces the notification with this ID.
    pub replaces: Option<u32>,
}

/// The user pressed a notification button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    /// The notification's ID.
    pub id: u32,
    /// The action key.
    pub action: String,
}

/// One process to start (LAUNCH-01 to LAUNCH-07).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchCommand {
    /// Program and arguments.
    pub argv: Vec<String>,
    /// Environment changes: `Some` sets, `None` removes.
    pub env: Vec<(String, Option<String>)>,
    /// Desktop ID of the app, for its systemd scope (LAUNCH-06).
    pub desktop_id: Option<String>,
    /// Display name, for the scope's description.
    pub name: String,
}

/// What the clipboard integration can do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClipboardCapabilities {
    /// Read on demand, and how.
    pub read: Option<&'static str>,
    /// Watch for changes, and how.
    pub watch: Option<&'static str>,
    /// Write, and how.
    pub write: Option<&'static str>,
}

/// A global shortcut as the mechanism reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundShortcut {
    /// Action ID, for example `toggle-menu`.
    pub action: String,
    /// Trigger as the mechanism describes it; `None` when unbound.
    pub trigger: Option<String>,
}

/// HTTP method for short-link expansion and Songlink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// `HEAD`, the default for expansion.
    Head,
    /// `GET`, the fallback, and Songlink.
    Get,
}

/// One HTTP request. Redirects are never followed and no cookies are kept;
/// the caller owns the redirect loop (DLG-EXP-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Method.
    pub method: HttpMethod,
    /// Absolute URL.
    pub url: String,
    /// Whole-request timeout.
    pub timeout: Duration,
    /// Read the body (Songlink); expansion leaves it unread.
    pub read_body: bool,
}

/// The parts of a response Wye uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// `Location` header.
    pub location: Option<String>,
    /// Body, when asked for.
    pub body: Option<String>,
}
