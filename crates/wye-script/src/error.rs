//! Turning what `QuickJS` throws into messages a user can act on: the line
//! the error points at (SCR-04, SCR-05), and the two limits (SCR-21).

use std::fmt;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Value};
use wye_core::hooks::ScriptError;

use crate::limits::Limits;

/// The name scripts are compiled under; stack traces point into it.
pub(crate) const SCRIPT_NAME: &str = "transform.js";

/// A script that does not compile (SCR-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// Counted from 1.
    pub line: u32,
    /// Counted from 1; 0 when the engine did not say.
    pub column: u32,
    pub message: String,
}

impl fmt::Display for SyntaxError {
    /// `line:column: message`, the `ScriptSyntax` D-Bus error's text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for SyntaxError {}

impl From<SyntaxError> for ScriptError {
    fn from(error: SyntaxError) -> Self {
        Self::at_line(error.message, error.line)
    }
}

/// Why a run stopped.
#[derive(Debug)]
pub(crate) enum Failure {
    /// `QuickJS` reported an error; for an exception, the thrown value is
    /// still pending in the context.
    Engine(rquickjs::Error),
    /// The script ran but its result is unusable.
    Script(ScriptError),
}

impl From<rquickjs::Error> for Failure {
    fn from(error: rquickjs::Error) -> Self {
        Self::Engine(error)
    }
}

impl From<ScriptError> for Failure {
    fn from(error: ScriptError) -> Self {
        Self::Script(error)
    }
}

/// What the run knew when it stopped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stop {
    /// The time limit passed (the interrupt handler fired).
    pub timed_out: bool,
    pub limits: Limits,
}

/// A thrown value, read out of the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Thrown {
    /// `SyntaxError: unexpected token`, or the thrown value as text.
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Explain `failure` to the user.
pub(crate) fn describe(ctx: &Ctx<'_>, failure: Failure, stop: Stop) -> ScriptError {
    match failure {
        Failure::Script(error) => error,
        Failure::Engine(rquickjs::Error::Exception) => {
            let thrown = thrown(ctx);
            let message = if stop.timed_out {
                time_limit_message(stop.limits)
            } else if is_out_of_memory(&thrown.message) {
                memory_limit_message(stop.limits)
            } else {
                thrown.message
            };
            ScriptError {
                message,
                line: thrown.line,
            }
        }
        Failure::Engine(rquickjs::Error::Allocation) => {
            ScriptError::new(memory_limit_message(stop.limits))
        }
        Failure::Engine(error) => ScriptError::new(error.to_string()),
    }
}

/// Read and clear the pending exception.
pub(crate) fn thrown(ctx: &Ctx<'_>) -> Thrown {
    let value = ctx.catch();
    match value.clone().into_exception() {
        Some(exception) => from_exception(&exception),
        None => Thrown {
            message: plain_value(&value),
            line: None,
            column: None,
        },
    }
}

fn from_exception(exception: &Exception<'_>) -> Thrown {
    let name: Option<String> = exception.get("name").ok();
    let message = exception.message().unwrap_or_default();
    let message = match name {
        Some(name) if !name.is_empty() && !message.is_empty() => format!("{name}: {message}"),
        Some(name) if !name.is_empty() => name,
        _ => message,
    };
    let (line, column) = exception
        .stack()
        .as_deref()
        .and_then(position_in_stack)
        .map_or((None, None), |(line, column)| (Some(line), column));
    // QuickJS-ng also sets `lineNumber`/`columnNumber` on syntax errors.
    let line = line.or_else(|| exception.get::<_, u32>("lineNumber").ok());
    let column = column.or_else(|| exception.get::<_, u32>("columnNumber").ok());
    Thrown {
        message,
        line,
        column,
    }
}

/// `throw "text"` and other values that are not errors.
fn plain_value(value: &Value<'_>) -> String {
    if value.is_null() || value.is_undefined() {
        // QuickJS throws nothing (an uncatchable null) when it runs out of
        // memory while building the error.
        return OUT_OF_MEMORY.to_owned();
    }
    value.get::<Coerced<String>>().map_or_else(
        |_| "the script threw a value".to_owned(),
        |text| format!("Uncaught {}", text.0),
    )
}

const OUT_OF_MEMORY: &str = "out of memory";

fn is_out_of_memory(message: &str) -> bool {
    message.contains(OUT_OF_MEMORY)
}

fn time_limit_message(limits: Limits) -> String {
    format!(
        "the script ran longer than {} ms and was stopped",
        limits.time.as_millis()
    )
}

fn memory_limit_message(limits: Limits) -> String {
    format!(
        "the script used more than {} MB of memory and was stopped",
        limits.memory / (1024 * 1024)
    )
}

/// The first `transform.js:<line>[:<column>]` in a stack trace.
pub(crate) fn position_in_stack(stack: &str) -> Option<(u32, Option<u32>)> {
    let marker = format!("{SCRIPT_NAME}:");
    let start = stack.find(&marker)? + marker.len();
    let rest = stack.get(start..)?;
    let mut numbers = rest
        .split(|c: char| !c.is_ascii_digit())
        .map(str::parse::<u32>);
    let line = numbers.next()?.ok()?;
    let column = rest
        .strip_prefix(&line.to_string())
        .and_then(|rest| rest.strip_prefix(':'))
        .and_then(|_| numbers.next())
        .and_then(Result::ok);
    Some((line, column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_positions_point_into_the_script() {
        assert_eq!(
            position_in_stack("    at transform (transform.js:4:12)\n    at <eval>"),
            Some((4, Some(12)))
        );
        assert_eq!(
            position_in_stack("    at transform.js:7\n"),
            Some((7, None))
        );
        assert_eq!(position_in_stack("    at <eval> (prelude:3:1)"), None);
    }

    #[test]
    fn syntax_errors_read_line_column_text() {
        let error = SyntaxError {
            line: 3,
            column: 14,
            message: "SyntaxError: unexpected token".into(),
        };
        assert_eq!(error.to_string(), "3:14: SyntaxError: unexpected token");
        assert_eq!(ScriptError::from(error).line, Some(3));
    }
}
