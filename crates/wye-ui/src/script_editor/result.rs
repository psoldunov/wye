//! The editor's result line (SCR-04, SCR-05, SCR-07): what a `RunScript`
//! answer shows, with the changed parts of the link marked.

use serde::Serialize;
use wye_api::scripts::ScriptRun;

/// The test link when there is no history (SCR-04).
pub const DEFAULT_TEST_URL: &str = "https://example.com/?utm_source=test";
/// The name `QuickJS` gives compile errors.
const SYNTAX_ERROR: &str = "SyntaxError";
/// Microseconds per millisecond.
const MICROS_PER_MILLI: u64 = 1000;
/// Below this many microseconds the time shows one decimal.
const DECIMAL_BELOW: u64 = 10 * MICROS_PER_MILLI;

/// What the result line says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing ran yet.
    Idle,
    Changed,
    Unchanged,
    Failed,
}

impl Status {
    /// The name QML switches on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::Failed => "failed",
        }
    }
}

/// A run of the result link, marked when the script changed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Segment {
    pub text: String,
    pub changed: bool,
}

/// Everything the result area shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultView {
    pub status: Status,
    /// The returned link, cut into marked and unmarked runs.
    pub segments: Vec<Segment>,
    /// The error, without its line.
    pub message: String,
    /// The line to mark, counted from 1; 0 for none (SCR-05).
    pub error_line: u32,
    /// The error is a compile error: Save is disabled (SCR-07).
    pub syntax_error: bool,
    /// "0.4 ms".
    pub time: String,
    /// `console.log` output, one line each.
    pub logs: String,
}

impl Default for ResultView {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            segments: Vec::new(),
            message: String::new(),
            error_line: 0,
            syntax_error: false,
            time: String::new(),
            logs: String::new(),
        }
    }
}

impl ResultView {
    /// The view of `run`.
    #[must_use]
    pub fn of(run: &ScriptRun) -> Self {
        let time = format_micros(run.micros);
        let logs = run.logs.join("\n");
        if !run.ok {
            let message = run.error.clone().unwrap_or_default();
            return Self {
                status: Status::Failed,
                syntax_error: is_syntax_error(&message),
                error_line: run.line.unwrap_or(0),
                message,
                time,
                logs,
                ..Self::default()
            };
        }
        match &run.url {
            Some(url) if !run.changed.is_empty() => Self {
                status: Status::Changed,
                segments: segments(url, &run.changed),
                time,
                logs,
                ..Self::default()
            },
            _ => Self {
                status: Status::Unchanged,
                time,
                logs,
                ..Self::default()
            },
        }
    }

    /// The view of a failure before the script ran (for example a test
    /// link that is not a link).
    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: Status::Failed,
            message: message.into(),
            ..Self::default()
        }
    }

    /// The segments as JSON for QML.
    #[must_use]
    pub fn segments_json(&self) -> String {
        serde_json::to_string(&self.segments).unwrap_or_else(|_| "[]".to_owned())
    }
}

/// Cut `url` at `ranges` (`[start, end)` in UTF-16 units). Ranges out of
/// order or out of bounds are clamped.
#[must_use]
pub fn segments(url: &str, ranges: &[[u32; 2]]) -> Vec<Segment> {
    let units: Vec<u16> = url.encode_utf16().collect();
    let len = units.len();
    let mut cursor = 0;
    let mut out = Vec::new();
    let push = |from: usize, to: usize, changed: bool, out: &mut Vec<Segment>| {
        if let Some(part) = units.get(from..to).filter(|part| !part.is_empty()) {
            out.push(Segment {
                text: String::from_utf16_lossy(part),
                changed,
            });
        }
    };
    for [start, end] in ranges {
        let start = usize::try_from(*start).unwrap_or(len).clamp(cursor, len);
        let end = usize::try_from(*end).unwrap_or(len).clamp(start, len);
        push(cursor, start, false, &mut out);
        push(start, end, true, &mut out);
        cursor = end;
    }
    push(cursor, len, false, &mut out);
    out
}

/// "0.4 ms" below 10 ms, "12 ms" above.
#[must_use]
pub fn format_micros(micros: u64) -> String {
    if micros < DECIMAL_BELOW {
        let tenths = (micros + 50) / 100;
        format!("{}.{} ms", tenths / 10, tenths % 10)
    } else {
        format!("{} ms", (micros + MICROS_PER_MILLI / 2) / MICROS_PER_MILLI)
    }
}

/// A compile error: the message `QuickJS` gives starts with `SyntaxError`.
#[must_use]
pub fn is_syntax_error(message: &str) -> bool {
    message.starts_with(SYNTAX_ERROR)
}

/// `SetScript`'s `ScriptSyntax` text, `line:column: message`: the line and
/// the message.
#[must_use]
pub fn parse_syntax_message(text: &str) -> (u32, String) {
    let mut parts = text.splitn(3, ':');
    let line = parts.next().and_then(|line| line.trim().parse().ok());
    let column = parts
        .next()
        .and_then(|column| column.trim().parse::<u32>().ok());
    match (line, column, parts.next()) {
        (Some(line), Some(_), Some(message)) => (line, message.trim().to_owned()),
        _ => (0, text.to_owned()),
    }
}

/// The test link: the last opened link from `GetHistory`, else
/// [`DEFAULT_TEST_URL`] (SCR-04).
#[must_use]
pub fn test_url(history_json: Option<&str>) -> String {
    history_json
        .and_then(|json| wye_api::json::decode::<wye_api::history::History>("History", json).ok())
        .and_then(|history| history.entries.into_iter().next())
        .map_or_else(|| DEFAULT_TEST_URL.to_owned(), |entry| entry.original_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(segments: &[Segment]) -> Vec<(String, bool)> {
        segments
            .iter()
            .map(|segment| (segment.text.clone(), segment.changed))
            .collect()
    }

    #[test]
    fn scr_04_changed_parts_are_marked() {
        let run = ScriptRun {
            ok: true,
            url: Some("https://x.com/a".into()),
            changed: vec![[8, 9]],
            micros: 420,
            logs: vec!["a".into(), "b".into()],
            ..ScriptRun::default()
        };
        let view = ResultView::of(&run);
        assert_eq!(view.status, Status::Changed);
        assert_eq!(
            texts(&view.segments),
            [
                ("https://".into(), false),
                ("x".into(), true),
                (".com/a".into(), false)
            ]
        );
        assert_eq!(view.time, "0.4 ms");
        assert_eq!(view.logs, "a\nb");
        assert!(view.segments_json().contains("\"changed\":true"));
    }

    #[test]
    fn scr_04_no_result_is_unchanged() {
        let kept = ResultView::of(&ScriptRun {
            ok: true,
            ..ScriptRun::default()
        });
        assert_eq!(kept.status, Status::Unchanged);
        let same = ResultView::of(&ScriptRun {
            ok: true,
            url: Some("https://a.example/".into()),
            ..ScriptRun::default()
        });
        assert_eq!(same.status, Status::Unchanged);
    }

    #[test]
    fn scr_05_and_07_errors_carry_their_line_and_kind() {
        let syntax = ResultView::of(&ScriptRun {
            error: Some("SyntaxError: unexpected token".into()),
            line: Some(2),
            ..ScriptRun::default()
        });
        assert_eq!(syntax.status, Status::Failed);
        assert_eq!(syntax.error_line, 2);
        assert!(syntax.syntax_error);
        let runtime = ResultView::of(&ScriptRun {
            error: Some("ReferenceError: x".into()),
            ..ScriptRun::default()
        });
        assert!(!runtime.syntax_error);
        assert_eq!(runtime.error_line, 0);
    }

    #[test]
    fn segments_survive_odd_ranges() {
        assert_eq!(
            texts(&segments("abc", &[[2, 1], [5, 9]])),
            [("ab".into(), false), ("c".into(), false)]
        );
        assert_eq!(
            texts(&segments("a\u{1f600}b", &[[1, 3]])),
            [
                ("a".into(), false),
                ("\u{1f600}".into(), true),
                ("b".into(), false)
            ]
        );
    }

    #[test]
    fn times_read_naturally() {
        assert_eq!(format_micros(0), "0.0 ms");
        assert_eq!(format_micros(1234), "1.2 ms");
        assert_eq!(format_micros(9_960), "10.0 ms");
        assert_eq!(format_micros(12_600), "13 ms");
    }

    #[test]
    fn syntax_messages_split_into_line_and_text() {
        assert_eq!(
            parse_syntax_message("2:14: SyntaxError: unexpected token"),
            (2, "SyntaxError: unexpected token".to_owned())
        );
        assert_eq!(parse_syntax_message("odd"), (0, "odd".to_owned()));
    }

    #[test]
    fn scr_04_the_test_link_is_the_last_opened_one() {
        assert_eq!(test_url(None), DEFAULT_TEST_URL);
        assert_eq!(test_url(Some("nope")), DEFAULT_TEST_URL);
    }
}
