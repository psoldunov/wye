//! Reading the Qt log of a self-test child and deciding what fails it.
//!
//! The child runs with [`MESSAGE_PATTERN`] as `QT_MESSAGE_PATTERN`, so each
//! message starts with a marker, its type and its category. Lines without
//! the marker continue the message before them; lines before the first
//! message come from something other than Qt's logger (fontconfig, Mesa)
//! and are shown but never fail the test.

/// Prefix of every Qt message in the child's stderr.
const MARKER: &str = "wye-log|";

/// `QT_MESSAGE_PATTERN` for the child.
pub const MESSAGE_PATTERN: &str = "wye-log|%{type}|%{category}|%{message}";

/// What the child prints once the surface loaded and handled its fixture.
pub const PASS_LINE: &str = "wye-ui self-test passed:";

/// Warnings that do not fail the self-test, each with the reason. Keep it
/// short: every entry hides a class of real mistakes.
const ALLOWED: &[(&str, &str)] = &[
    (
        "is not a wayland window. Not creating zwlr_layer_surface",
        "LayerShellQt under QT_QPA_PLATFORM=offscreen: the picker's layer-shell properties are ignored, the window still loads",
    ),
    (
        "This plugin does not support raise()",
        "the offscreen platform cannot raise windows; SET-04's raise works on Wayland and X11",
    ),
    (
        "Could not find any platform plugin",
        "KWindowSystem has plugins for Wayland and X11 only; the shim's calls fall back as designed (design B)",
    ),
    (
        "QStandardPaths: XDG_RUNTIME_DIR not set",
        "the Nix build sandbox has no session",
    ),
    (
        "QStandardPaths: runtime directory",
        "the Nix build sandbox's temporary runtime directory has loose permissions",
    ),
    (
        "Icon theme \"breeze-internal\" not found",
        "Kirigami 6.30 as Debian and Fedora package it asks for Breeze's built-in fallback theme, which their breeze-icons does not register under offscreen; icons still come from the installed Breeze theme (packaging/smoke-test.sh)",
    ),
];

/// A Qt message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Critical,
    Fatal,
}

impl Level {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "critical" => Some(Self::Critical),
            "fatal" => Some(Self::Fatal),
            _ => None,
        }
    }

    /// Whether a message of this type fails the test unless allowed.
    const fn is_problem(self) -> bool {
        matches!(self, Self::Warning | Self::Critical | Self::Fatal)
    }
}

/// One Qt message, continuation lines included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub level: Level,
    pub category: String,
    pub text: String,
}

/// The child's stderr, split up.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Log {
    /// Qt messages in order.
    pub messages: Vec<Message>,
    /// Lines that are not Qt messages.
    pub foreign: Vec<String>,
}

impl Log {
    /// Split `stderr` into messages.
    pub fn parse(stderr: &str) -> Self {
        stderr.lines().fold(Self::default(), Self::push_line)
    }

    fn push_line(self, line: &str) -> Self {
        let Self {
            mut messages,
            mut foreign,
        } = self;
        match (parse_message(line), messages.last_mut()) {
            (Some(message), _) => messages.push(message),
            (None, Some(last)) => {
                last.text.push('\n');
                last.text.push_str(line);
            }
            (None, None) => foreign.push(line.to_owned()),
        }
        Self { messages, foreign }
    }

    /// Warnings and errors outside the allow-list.
    pub fn problems(&self) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|message| message.level.is_problem() && !is_allowed(&message.text))
            .collect()
    }

    /// Whether the child printed [`PASS_LINE`].
    pub fn passed(&self) -> bool {
        self.messages
            .iter()
            .any(|message| message.text.starts_with(PASS_LINE))
    }
}

fn parse_message(line: &str) -> Option<Message> {
    let mut fields = line.strip_prefix(MARKER)?.splitn(3, '|');
    let level = Level::parse(fields.next()?)?;
    let category = fields.next()?.to_owned();
    let text = fields.next()?.to_owned();
    Some(Message {
        level,
        category,
        text,
    })
}

fn is_allowed(text: &str) -> bool {
    ALLOWED
        .iter()
        .any(|(needle, _reason)| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_keep_their_continuation_lines() {
        let log = Log::parse(
            "Fontconfig warning: no config\n\
             wye-log|warning|qml|file.qml:3: TypeError: x is undefined\n\
             \tat line 4\n\
             wye-log|info|js|wye-ui self-test passed: picker\n",
        );
        assert_eq!(log.foreign, ["Fontconfig warning: no config"]);
        assert_eq!(log.messages.len(), 2);
        assert_eq!(
            log.messages[0].text,
            "file.qml:3: TypeError: x is undefined\n\tat line 4"
        );
        assert_eq!(log.messages[0].category, "qml");
        assert!(log.passed());
    }

    #[test]
    fn warnings_fail_unless_allowed() {
        let log = Log::parse(
            "wye-log|warning|default|QQuickWindow(0x1) is not a wayland window. Not creating zwlr_layer_surface\n\
             wye-log|debug|qml|chatter\n\
             wye-log|critical|qml|Main.qml:1 module \"org.kde.kirigami\" is not installed\n",
        );
        let problems = log.problems();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].level, Level::Critical);
        assert!(!log.passed());
    }

    #[test]
    fn a_message_with_pipes_in_its_text_stays_whole() {
        let log = Log::parse("wye-log|warning|qml|a | b | c\n");
        assert_eq!(log.messages[0].text, "a | b | c");
    }

    #[test]
    fn every_allowance_has_a_reason() {
        assert!(
            ALLOWED
                .iter()
                .all(|(needle, reason)| !needle.is_empty() && !reason.is_empty())
        );
    }
}
