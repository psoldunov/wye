//! Reading the `GLib` log of a self-test child and deciding what fails it.
//!
//! The child installs a structured `GLib` log writer ([`install_writer`])
//! that prints every message of level `message` or worse as one line:
//! [`MARKER`], then the message as JSON (level, log domain, text and, when
//! `GLib` knows it, the code that logged it). GTK's, libadwaita's and
//! `GLib`'s warnings and criticals all arrive there, a message of several
//! lines still on one. Every other line comes from elsewhere (Rust's
//! `tracing`, fontconfig): it is shown but never fails the test.
//!
//! Any warning or critical fails the run unless [`ALLOWED`] names it: a
//! message GTK logs about its own state, whatever Wye does.

use std::io::Write as _;

use gtk::glib;
use serde::{Deserialize, Serialize};

/// Prefix of every `GLib` message in the child's stderr.
const MARKER: &str = "wye-log ";

/// Warnings that do not fail the self-test: log domain, text the message
/// contains, and the reason. Keep it short: every entry hides a class of
/// real mistakes, so never add one for a warning Wye's code causes.
const ALLOWED: &[(&str, &str, &str)] = &[(
    "Gdk",
    "gdk_frame_timings_submitted() called on submitted frame.",
    "GTK 4.24's X11 backend reports one frame as submitted twice (gdk/x11/gdksurface-x11.c and \
     gdk/x11/gdkdisplay-x11.c both call gdk_frame_clock_submitted()), once per process, on \
     Xvfb; Wye never touches frame timings (Debian testing, packaging/smoke-test.sh)",
)];

/// What the child prints once the surface showed every case.
pub const PASS_LINE: &str = "wye-gtk self-test passed:";

/// A `GLib` log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Message,
    Warning,
    Critical,
    Error,
}

impl Level {
    const fn from_glib(level: glib::LogLevel) -> Self {
        match level {
            glib::LogLevel::Error => Self::Error,
            glib::LogLevel::Critical => Self::Critical,
            glib::LogLevel::Warning => Self::Warning,
            glib::LogLevel::Message => Self::Message,
            glib::LogLevel::Info => Self::Info,
            glib::LogLevel::Debug => Self::Debug,
        }
    }

    /// Whether a message of this level fails the test.
    const fn is_problem(self) -> bool {
        matches!(self, Self::Warning | Self::Critical | Self::Error)
    }
}

/// One `GLib` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub level: Level,
    pub domain: String,
    pub text: String,
    /// `CODE_FILE:CODE_LINE` of the code that logged it; empty when unknown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
}

impl Message {
    /// The message as one line of a failure report.
    pub fn report(&self) -> String {
        let at = if self.code.is_empty() {
            String::new()
        } else {
            format!(" (at {})", self.code)
        };
        format!("{:?} [{}] {}{at}", self.level, self.domain, self.text)
    }

    /// Whether [`ALLOWED`] names this message.
    fn is_allowed(&self) -> bool {
        ALLOWED
            .iter()
            .any(|(domain, needle, _reason)| self.domain == *domain && self.text.contains(needle))
    }
}

/// The child's stderr, split up.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Log {
    /// `GLib` messages in order.
    pub messages: Vec<Message>,
    /// Lines that are not `GLib` messages.
    pub foreign: Vec<String>,
}

impl Log {
    /// Split `stderr` into messages and the rest.
    pub fn parse(stderr: &str) -> Self {
        let mut log = Self::default();
        for line in stderr.lines() {
            let message = line
                .strip_prefix(MARKER)
                .and_then(|json| serde_json::from_str::<Message>(json).ok());
            match message {
                Some(message) => log.messages.push(message),
                None => log.foreign.push(line.to_owned()),
            }
        }
        log
    }

    /// The warnings, criticals and errors outside [`ALLOWED`].
    pub fn problems(&self) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|message| message.level.is_problem() && !message.is_allowed())
            .collect()
    }

    /// Whether the child printed [`PASS_LINE`].
    pub fn passed(&self) -> bool {
        self.messages
            .iter()
            .any(|message| message.text.starts_with(PASS_LINE))
    }
}

/// Print `message` as a marked line on stderr.
fn emit(message: &Message) {
    let Ok(json) = serde_json::to_string(message) else {
        return;
    };
    // stderr is gone only when the parent is: nobody left to tell.
    let _ = writeln!(std::io::stderr().lock(), "{MARKER}{json}");
}

/// Print `text` as a message of `level` from `domain`.
pub fn print(level: Level, domain: &str, text: &str) {
    emit(&Message {
        level,
        domain: domain.to_owned(),
        text: text.to_owned(),
        code: String::new(),
    });
}

/// Send every `GLib` message of level `message` or worse to stderr as a
/// marked line (see the module docs). Call it before GTK starts.
pub fn install_writer() {
    glib::log_set_writer_func(|level, fields| {
        let level = Level::from_glib(level);
        if matches!(level, Level::Debug | Level::Info) {
            return glib::LogWriterOutput::Handled;
        }
        let field = |key: &str| {
            fields
                .iter()
                .find(|field| field.key() == key)
                .and_then(|field| field.value_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let (file, line) = (field("CODE_FILE"), field("CODE_LINE"));
        emit(&Message {
            level,
            domain: field("GLIB_DOMAIN"),
            text: field("MESSAGE"),
            code: if file.is_empty() {
                String::new()
            } else {
                format!("{file}:{line}")
            },
        });
        glib::LogWriterOutput::Handled
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(level: &str, domain: &str, text: &str) -> String {
        let json = serde_json::json!({"level": level, "domain": domain, "text": text});
        format!("{MARKER}{json}\n")
    }

    #[test]
    fn a_message_of_several_lines_is_one_message() {
        let stderr = format!(
            "Fontconfig warning: no config\n{}{}",
            line(
                "warning",
                "Gtk",
                "Allocating size to GtkBox\n\twithout calling measure"
            ),
            line("message", "wye-gtk", "wye-gtk self-test passed: about"),
        );
        let log = Log::parse(&stderr);
        assert_eq!(log.foreign, ["Fontconfig warning: no config"]);
        let texts: Vec<_> = log.messages.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Allocating size to GtkBox\n\twithout calling measure",
                "wye-gtk self-test passed: about"
            ]
        );
        assert!(log.passed());
        assert_eq!(log.problems().len(), 1);
    }

    #[test]
    fn criticals_fail_and_messages_do_not() {
        let stderr = line("message", "Gtk", "chatter")
            + &line(
                "critical",
                "Adwaita",
                "adw_dialog_present: assertion failed",
            );
        let log = Log::parse(&stderr);
        let levels: Vec<_> = log.problems().iter().map(|m| m.level).collect();
        assert_eq!(levels, [Level::Critical]);
        assert!(!log.passed());
    }

    #[test]
    fn an_allowed_warning_passes_only_from_its_own_domain() {
        let text = "gdk_frame_timings_submitted() called on submitted frame.";
        let stderr = line("warning", "Gdk", text)
            + &line("message", "wye-gtk", "wye-gtk self-test passed: picker");
        let log = Log::parse(&stderr);
        assert!(log.passed());
        assert!(log.problems().is_empty());

        let elsewhere = Log::parse(&line("warning", "Gtk", text));
        assert_eq!(elsewhere.problems().len(), 1);
    }

    #[test]
    fn a_report_names_the_code_when_glib_knows_it() {
        let message = Message {
            level: Level::Warning,
            domain: "Gtk".to_owned(),
            text: "oops".to_owned(),
            code: "gtkwidget.c:42".to_owned(),
        };
        assert_eq!(message.report(), "Warning [Gtk] oops (at gtkwidget.c:42)");
        let printed = format!("{MARKER}{}", serde_json::to_string(&message).expect("json"));
        assert_eq!(Log::parse(&printed).messages, [message]);
    }

    #[test]
    fn a_marked_line_that_is_not_json_is_foreign() {
        let log = Log::parse("wye-log not json\n");
        assert!(log.messages.is_empty());
        assert_eq!(log.foreign.len(), 1);
    }
}
