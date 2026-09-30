//! `Exec` command lines: tokenising, field codes and where the URL and
//! launch flags go ([Desktop Entry Specification, "The Exec key"]).
//!
//! The value handed to [`ExecTemplate::parse`] must already have had the
//! key file's string escapes applied (as [`crate::DesktopEntry::exec`] has);
//! the specification applies those before the quoting rules here.
//!
//! [Desktop Entry Specification, "The Exec key"]: https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html

use std::path::Path;

use crate::entry::DesktopEntry;

/// Flatpak's markers around forwarded URIs (`@@u %u @@`) and files
/// (`@@ %f @@`). Launch flags go in front of the opening marker so the
/// markers keep enclosing only the URL.
const FLATPAK_OPEN_MARKERS: [&str; 2] = ["@@u", "@@"];

/// A tokenised `Exec` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecTemplate {
    args: Vec<Arg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Arg {
    pieces: Vec<Piece>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    /// `%u`, `%U`, `%f` or `%F`: the first one receives the URL.
    Url,
    /// `%i`.
    Icon,
    /// `%c`.
    Name,
    /// `%k`.
    Location,
    /// Deprecated, repeated or unknown field codes, which expand to nothing.
    Dropped,
}

/// What `%i`, `%c` and `%k` expand to.
#[derive(Debug, Clone, Copy)]
pub struct ExecContext<'a> {
    pub name: &'a str,
    pub icon: Option<&'a str>,
    pub location: &'a Path,
}

impl<'a> ExecContext<'a> {
    #[must_use]
    pub fn from_entry(entry: &'a DesktopEntry) -> Self {
        Self {
            name: &entry.name,
            icon: entry.icon.as_deref(),
            location: &entry.path,
        }
    }
}

/// An `Exec` value that cannot be tokenised.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecError {
    #[error("Exec has an unterminated double quote")]
    UnterminatedQuote,
    #[error("Exec is empty")]
    Empty,
}

impl ExecTemplate {
    /// Tokenises an `Exec` value.
    ///
    /// Outside quotes, arguments split on whitespace and a backslash takes
    /// the next character literally. Inside double quotes, `\"`, `` \` ``,
    /// `\$` and `\\` are escapes. `%%` is a literal `%`.
    ///
    /// # Errors
    ///
    /// Returns [`ExecError`] for an unterminated quote or an empty line.
    pub fn parse(exec: &str) -> Result<Self, ExecError> {
        let mut tokenizer = Tokenizer::default();
        let mut chars = exec.chars();
        while let Some(c) = chars.next() {
            tokenizer.feed(c, &mut chars);
        }
        tokenizer.finish()
    }

    /// True when the line has a `%u`/`%U`/`%f`/`%F` field code.
    #[must_use]
    pub fn has_url_field(&self) -> bool {
        self.url_index().is_some()
    }

    /// The literal text of every argument with field codes left out, for
    /// inspecting the program and its options (DISC-04).
    #[must_use]
    pub fn words(&self) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| {
                arg.pieces
                    .iter()
                    .filter_map(|piece| match piece {
                        Piece::Text(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect()
            })
            .collect()
    }

    /// Expands the line into a program and its arguments (LAUNCH-01).
    ///
    /// `flags` go immediately before the URL argument, or before Flatpak's
    /// `@@u` marker when one precedes it. Without a URL field code, the flags
    /// and then the URL are appended.
    #[must_use]
    pub fn expand(&self, context: &ExecContext<'_>, url: &str, flags: &[String]) -> Vec<String> {
        let url_index = self.url_index();
        let before_url = url_index.and_then(|index| index.checked_sub(1));
        let insert_at = match before_url {
            Some(marker) if self.args.get(marker).is_some_and(Arg::is_flatpak_marker) => marker,
            _ => url_index.unwrap_or(self.args.len()),
        };
        let mut out = Vec::with_capacity(self.args.len() + flags.len() + 1);
        for (index, arg) in self.args.iter().enumerate() {
            if index == insert_at {
                out.extend(flags.iter().cloned());
            }
            arg.expand_into(context, url, &mut out);
        }
        if insert_at == self.args.len() {
            out.extend(flags.iter().cloned());
        }
        if url_index.is_none() {
            out.push(url.to_owned());
        }
        out
    }

    fn url_index(&self) -> Option<usize> {
        self.args
            .iter()
            .position(|arg| arg.pieces.contains(&Piece::Url))
    }
}

impl Arg {
    fn is_flatpak_marker(&self) -> bool {
        matches!(self.pieces.as_slice(), [Piece::Text(text)] if FLATPAK_OPEN_MARKERS.contains(&text.as_str()))
    }

    fn expand_into(&self, context: &ExecContext<'_>, url: &str, out: &mut Vec<String>) {
        if self.pieces == [Piece::Icon] {
            if let Some(icon) = context.icon.filter(|icon| !icon.is_empty()) {
                out.push("--icon".to_owned());
                out.push(icon.to_owned());
            }
            return;
        }
        let mut value = String::new();
        let mut has_text = false;
        for piece in &self.pieces {
            match piece {
                Piece::Text(text) => {
                    has_text = true;
                    value.push_str(text);
                }
                Piece::Url => value.push_str(url),
                Piece::Icon => value.push_str(context.icon.unwrap_or_default()),
                Piece::Name => value.push_str(context.name),
                Piece::Location => value.push_str(&context.location.to_string_lossy()),
                Piece::Dropped => {}
            }
        }
        // An argument made only of field codes that expanded to nothing is
        // removed; a literal empty argument (`""`) is kept.
        if has_text || !value.is_empty() {
            out.push(value);
        }
    }
}

#[derive(Default)]
struct Tokenizer {
    args: Vec<Arg>,
    pieces: Vec<Piece>,
    text: String,
    started: bool,
    in_quotes: bool,
    url_seen: bool,
}

impl Tokenizer {
    fn feed(&mut self, c: char, rest: &mut std::str::Chars<'_>) {
        if !self.in_quotes && c.is_ascii_whitespace() {
            self.end_arg();
            return;
        }
        self.started = true;
        match c {
            '"' => self.in_quotes = !self.in_quotes,
            '\\' => match rest.next() {
                // Inside quotes only `"`, `` ` ``, `$` and `\` are escapes.
                Some(next) if self.in_quotes && !matches!(next, '"' | '`' | '$' | '\\') => {
                    self.text.push('\\');
                    self.text.push(next);
                }
                Some(next) => self.text.push(next),
                None => self.text.push('\\'),
            },
            '%' => match rest.next() {
                // `%%`, or a lone `%` at the end, is a literal percent sign.
                Some('%') | None => self.text.push('%'),
                Some(code) => {
                    let piece = self.field_code(code);
                    self.push_text();
                    self.pieces.push(piece);
                }
            },
            _ => self.text.push(c),
        }
    }

    fn field_code(&mut self, code: char) -> Piece {
        match code {
            'u' | 'U' | 'f' | 'F' if !self.url_seen => {
                self.url_seen = true;
                Piece::Url
            }
            'i' => Piece::Icon,
            'c' => Piece::Name,
            'k' => Piece::Location,
            // %d %D %n %N %v %m are deprecated; anything else is invalid.
            _ => Piece::Dropped,
        }
    }

    fn push_text(&mut self) {
        if !self.text.is_empty() {
            self.pieces
                .push(Piece::Text(std::mem::take(&mut self.text)));
        }
    }

    fn end_arg(&mut self) {
        if !self.started {
            return;
        }
        if self.pieces.is_empty() || !self.text.is_empty() {
            self.pieces
                .push(Piece::Text(std::mem::take(&mut self.text)));
        }
        self.args.push(Arg {
            pieces: std::mem::take(&mut self.pieces),
        });
        self.started = false;
    }

    fn finish(mut self) -> Result<ExecTemplate, ExecError> {
        if self.in_quotes {
            return Err(ExecError::UnterminatedQuote);
        }
        self.end_arg();
        if self.args.is_empty() {
            return Err(ExecError::Empty);
        }
        Ok(ExecTemplate { args: self.args })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const URL: &str = "https://example.com/a b";

    fn context(path: &Path) -> ExecContext<'_> {
        ExecContext {
            name: "Firefox",
            icon: Some("firefox"),
            location: path,
        }
    }

    fn run(exec: &str, flags: &[&str]) -> Vec<String> {
        let path = PathBuf::from("/apps/firefox.desktop");
        let flags: Vec<String> = flags.iter().map(|f| (*f).to_owned()).collect();
        ExecTemplate::parse(exec)
            .unwrap()
            .expand(&context(&path), URL, &flags)
    }

    #[test]
    fn places_url_and_flags() {
        assert_eq!(
            run("firefox --name firefox %u", &["--private-window"]),
            vec!["firefox", "--name", "firefox", "--private-window", URL]
        );
        assert_eq!(
            run("/usr/bin/google-chrome-stable %U", &[]),
            vec!["/usr/bin/google-chrome-stable", URL]
        );
        assert_eq!(
            run("app --flag", &["--new-window"]),
            vec!["app", "--flag", "--new-window", URL]
        );
        assert_eq!(
            run("app --url=%u", &[]),
            vec!["app".to_owned(), format!("--url={URL}")]
        );
    }

    #[test]
    fn keeps_flatpak_markers_around_url() {
        let exec = "/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox \
                    --file-forwarding org.mozilla.firefox @@u %u @@";
        let args = run(exec, &["--profile", "/p"]);
        let tail: Vec<&str> = args
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            vec!["org.mozilla.firefox", "--profile", "/p", "@@u", URL, "@@"]
        );
    }

    #[test]
    fn handles_quotes_and_escapes() {
        assert_eq!(
            run(r#""/opt/My App/app" "a \"q\" \$x \\ \n" %u"#, &[]),
            vec!["/opt/My App/app", r#"a "q" $x \ \n"#, URL]
        );
        assert_eq!(run(r#"app "" %u"#, &[]), vec!["app", "", URL]);
        assert_eq!(run(r"app a\ b 100%%", &[]), vec!["app", "a b", "100%", URL]);
        assert_eq!(
            ExecTemplate::parse(r#"app "open"#),
            Err(ExecError::UnterminatedQuote)
        );
        assert_eq!(ExecTemplate::parse("   "), Err(ExecError::Empty));
    }

    #[test]
    fn expands_other_field_codes() {
        assert_eq!(
            run("app %i --title %c --from %k %u", &[]),
            vec![
                "app",
                "--icon",
                "firefox",
                "--title",
                "Firefox",
                "--from",
                "/apps/firefox.desktop",
                URL
            ]
        );
        let path = PathBuf::from("/x.desktop");
        let no_icon = ExecContext {
            icon: None,
            ..context(&path)
        };
        let args = ExecTemplate::parse("app %i %u")
            .unwrap()
            .expand(&no_icon, URL, &[]);
        assert_eq!(args, vec!["app", URL]);
    }

    #[test]
    fn drops_deprecated_and_repeated_codes() {
        assert_eq!(run("app %d %D %n %N %v %m %u %U", &[]), vec!["app", URL]);
        assert_eq!(run("app %f", &[]), vec!["app", URL]);
    }

    #[test]
    fn reports_words() {
        let template = ExecTemplate::parse("env A=1 /usr/bin/brave %U").unwrap();
        assert_eq!(template.words(), vec!["env", "A=1", "/usr/bin/brave", ""]);
        assert!(template.has_url_field());
    }
}
