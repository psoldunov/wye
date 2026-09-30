//! Running one transform script (16-script-editor.md, "Script API").
//!
//! Every call gets a fresh `QuickJS` runtime with a memory limit and an
//! interrupt handler that stops it at the time limit (SCR-21), so one
//! script can never affect the next. The script is compiled as an ES module
//! and its `default` export is called with the URL and the context.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rquickjs::{Context, Ctx, Module, Object, Runtime, Value};
use url::Url;
use wye_core::hooks::{ScriptError, TransformContext};

use crate::error::{Failure, SCRIPT_NAME, Stop, SyntaxError, describe, thrown};
use crate::limits::{Limits, MAX_LOG_CHARS, MAX_LOG_LINES};
use crate::prelude::{self, Helpers};

/// What the script's `context` argument holds (Script API).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptInput {
    /// `context.sourceApp`.
    pub source_app: Option<String>,
    /// `context.entryPoint`: `handler`, `clipboard`, `extension` or `cli`.
    pub entry_point: String,
    /// `context.heldKeys`, for example `["Ctrl"]`.
    pub held_keys: Vec<String>,
    /// `context.rule`: the matched rule's name, per-rule scripts only.
    pub rule: Option<String>,
}

impl ScriptInput {
    /// The input the pipeline describes.
    #[must_use]
    pub fn from_context(context: &TransformContext<'_>) -> Self {
        Self {
            source_app: context.source_app(),
            entry_point: context.entry.script_name().to_owned(),
            held_keys: context.held_keys().into_iter().map(str::to_owned).collect(),
            rule: context.rule.map(|rule| rule.name.to_owned()),
        }
    }
}

/// The outcome of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// `Ok(None)`: the script kept the link. `Ok(Some(url))`: it returned
    /// `url`, which may equal the input. `Err`: it failed (SCR-21 to
    /// SCR-23).
    pub result: Result<Option<Url>, ScriptError>,
    /// `console.log` lines, oldest first (SCR-20).
    pub logs: Vec<String>,
    /// Wall time of the whole run, runtime set-up included.
    pub elapsed: Duration,
}

/// Run `source` over `url` with the default limits.
#[must_use]
pub fn run(source: &str, url: &Url, input: &ScriptInput) -> Run {
    run_with_limits(source, url, input, Limits::default())
}

/// Run `source` over `url` with `limits`.
#[must_use]
pub fn run_with_limits(source: &str, url: &Url, input: &ScriptInput, limits: Limits) -> Run {
    let started = Instant::now();
    let logs = Rc::new(RefCell::new(Logs::default()));
    let result = execute(source, url, input, limits, &logs);
    let logs = logs.take().into_lines();
    Run {
        result,
        logs,
        elapsed: started.elapsed(),
    }
}

/// Compile `source` without running it (SCR-07).
///
/// # Errors
///
/// [`SyntaxError`] with the position `QuickJS` reports.
pub fn check(source: &str) -> Result<(), SyntaxError> {
    let limits = Limits::default();
    let runtime = new_runtime(limits).map_err(|error| engine_syntax_error(&error))?;
    let context = Context::full(&runtime).map_err(|error| engine_syntax_error(&error))?;
    context.with(
        |ctx| match Module::declare(ctx.clone(), SCRIPT_NAME, source) {
            Ok(_) => Ok(()),
            Err(rquickjs::Error::Exception) => {
                let thrown = thrown(&ctx);
                Err(SyntaxError {
                    line: thrown.line.unwrap_or(1),
                    column: thrown.column.unwrap_or(0),
                    message: thrown.message,
                })
            }
            Err(error) => Err(engine_syntax_error(&error)),
        },
    )
}

fn engine_syntax_error(error: &rquickjs::Error) -> SyntaxError {
    SyntaxError {
        line: 1,
        column: 0,
        message: error.to_string(),
    }
}

fn new_runtime(limits: Limits) -> rquickjs::Result<Runtime> {
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(limits.memory);
    runtime.set_max_stack_size(limits.stack);
    Ok(runtime)
}

fn execute(
    source: &str,
    url: &Url,
    input: &ScriptInput,
    limits: Limits,
    logs: &Rc<RefCell<Logs>>,
) -> Result<Option<Url>, ScriptError> {
    let runtime = new_runtime(limits).map_err(|error| ScriptError::new(error.to_string()))?;
    let deadline: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    // Set only when the handler really stopped the script: a script that
    // threw on its own, however slowly the machine ran it, keeps its own
    // error (SCR-21, SCR-22).
    let interrupted = Rc::new(Cell::new(false));
    let armed = Rc::clone(&deadline);
    let stopped = Rc::clone(&interrupted);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let late = armed
            .get()
            .is_some_and(|deadline| Instant::now() >= deadline);
        if late {
            stopped.set(true);
        }
        late
    })));
    let context = Context::full(&runtime).map_err(|error| ScriptError::new(error.to_string()))?;
    context.with(|ctx| {
        let sink = Rc::clone(logs);
        let outcome = prelude::install(&ctx, move |line| sink.borrow_mut().push(line))
            .map_err(Failure::from)
            .and_then(|helpers| {
                // The clock starts with the user's code (SCR-21).
                deadline.set(Some(Instant::now() + limits.time));
                call(&ctx, &helpers, source, url, input)
            });
        outcome.map_err(|failure| {
            let timed_out = interrupted.get();
            describe(&ctx, failure, Stop { timed_out, limits })
        })
    })
}

/// Evaluate the module and call its default export.
fn call<'js>(
    ctx: &Ctx<'js>,
    helpers: &Helpers<'js>,
    source: &str,
    url: &Url,
    input: &ScriptInput,
) -> Result<Option<Url>, Failure> {
    let (module, evaluated) = Module::declare(ctx.clone(), SCRIPT_NAME, source)?.eval()?;
    evaluated.finish::<()>()?;
    let export: Value = module.get("default")?;
    let Some(transform) = export.as_function() else {
        return Err(ScriptError::new(
            "the script has no default export function; start it with \
             `export default function transform(url, context) {`",
        )
        .into());
    };
    let argument: Value = helpers.make.call((url.as_str(),))?;
    let returned: Value = transform.call((argument, context_object(ctx, input)?))?;
    let returned = match returned.as_promise() {
        Some(promise) => promise.finish::<Value>().map_err(|error| match error {
            // Nothing left to run and the promise still pending (SCR-23).
            rquickjs::Error::WouldBlock => Failure::from(ScriptError::new(
                "the script's promise never settled; resolve it with a URL, a string or nothing",
            )),
            other => Failure::from(other),
        })?,
        None => returned,
    };
    interpret(helpers, &returned, url)
}

fn context_object<'js>(ctx: &Ctx<'js>, input: &ScriptInput) -> rquickjs::Result<Object<'js>> {
    let null = || Value::new_null(ctx.clone());
    let text = |value: &Option<String>| -> rquickjs::Result<Value<'js>> {
        match value {
            Some(text) => rquickjs::String::from_str(ctx.clone(), text).map(Into::into),
            None => Ok(null()),
        }
    };
    let object = Object::new(ctx.clone())?;
    object.set("sourceApp", text(&input.source_app)?)?;
    object.set("entryPoint", input.entry_point.as_str())?;
    object.set("heldKeys", input.held_keys.clone())?;
    object.set("rule", text(&input.rule)?)?;
    Ok(object)
}

/// What the default export returned (SCR-23).
fn interpret<'js>(
    helpers: &Helpers<'js>,
    returned: &Value<'js>,
    original: &Url,
) -> Result<Option<Url>, Failure> {
    if returned.is_undefined() || returned.is_null() {
        return Ok(None);
    }
    let text = if let Some(text) = returned.as_string() {
        text.to_string()?
    } else {
        let href: Option<String> = helpers.href_of.call((returned.clone(),))?;
        href.ok_or_else(|| {
            ScriptError::new(format!(
                "the script returned a value of type {}; return a URL, a string or nothing",
                js_type(returned)
            ))
        })?
    };
    accept(&text, original).map(Some).map_err(Failure::from)
}

/// The type roughly as JavaScript's `typeof` names it.
fn js_type(value: &Value<'_>) -> &'static str {
    match value.type_name() {
        "int" | "float" => "number",
        "bool" => "boolean",
        other => other,
    }
}

/// SCR-23: the result must be an `http`/`https` link. Handing back the
/// link unchanged is always fine (local HTML files, DEF-07).
fn accept(text: &str, original: &Url) -> Result<Url, ScriptError> {
    let url = Url::parse(text).map_err(|error| {
        ScriptError::new(format!(
            "the script returned \u{201c}{text}\u{201d}, which is not a link: {error}"
        ))
    })?;
    let web = matches!(url.scheme(), "http" | "https") && url.has_host();
    if web || url == *original {
        Ok(url)
    } else {
        Err(ScriptError::new(format!(
            "the script returned {url}, which is not an http or https link"
        )))
    }
}

/// `console.log` output, capped (SCR-20).
#[derive(Debug, Default)]
struct Logs {
    lines: Vec<String>,
    dropped: usize,
}

impl Logs {
    fn push(&mut self, line: String) {
        if self.lines.len() < MAX_LOG_LINES {
            self.lines.push(truncate(line));
        } else {
            self.dropped += 1;
        }
    }

    fn into_lines(self) -> Vec<String> {
        let dropped = (self.dropped > 0).then(|| format!("\u{2026} {} more lines", self.dropped));
        self.lines.into_iter().chain(dropped).collect()
    }
}

fn truncate(line: String) -> String {
    match line.char_indices().nth(MAX_LOG_CHARS) {
        Some((end, _)) => line
            .get(..end)
            .map_or_else(String::new, |kept| format!("{kept}\u{2026}")),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_log_lines_are_cut() {
        let line = "x".repeat(MAX_LOG_CHARS + 5);
        let cut = truncate(line);
        assert_eq!(cut.chars().count(), MAX_LOG_CHARS + 1);
        assert!(cut.ends_with('\u{2026}'));
    }

    #[test]
    fn extra_log_lines_are_counted() {
        let mut logs = Logs::default();
        for index in 0..MAX_LOG_LINES + 3 {
            logs.push(index.to_string());
        }
        let lines = logs.into_lines();
        assert_eq!(lines.len(), MAX_LOG_LINES + 1);
        assert_eq!(
            lines.last().map(String::as_str),
            Some("\u{2026} 3 more lines")
        );
    }

    #[test]
    fn a_returned_file_link_must_be_the_original() {
        let file = Url::parse("file:///tmp/a.html").expect("url");
        assert!(accept("file:///tmp/a.html", &file).is_ok());
        assert!(accept("file:///tmp/b.html", &file).is_err());
        assert!(accept("mailto:a@b.c", &file).is_err());
    }
}
