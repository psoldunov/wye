//! The one-shot query script: the template in `data/kwin/wye-query.js`, its
//! placeholders, and the file `KWin` loads it from.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use crate::platform::PlatformError;

/// The script, with `@SERVICE@`, `@PATH@`, `@INTERFACE@` and `@NONCE@` to
/// fill in.
pub const TEMPLATE: &str = include_str!("../../../../../data/kwin/wye-query.js");

/// Prefix of the name each query's script is loaded under; the nonce
/// follows.
pub const NAME_PREFIX: &str = "wye-query-";

/// Directory permissions: the user only.
const DIR_MODE: u32 = 0o700;

/// File permissions: the user only.
const FILE_MODE: u32 = 0o600;

/// Where the script's answer goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTo {
    /// Bus name of the connection serving `KWin1`: its unique name, so the
    /// answer reaches this process even when another owns
    /// `dev.soldunov.wye` (`wye debug probe` beside a running service).
    pub service: String,
    /// Object path serving `KWin1`.
    pub path: String,
    /// The interface, `dev.soldunov.wye.KWin1`.
    pub interface: String,
}

/// The script for one query.
///
/// # Errors
///
/// [`PlatformError::Failed`] when a value holds a character that could end
/// the JavaScript string it goes into. Bus names, object paths and nonces
/// never do; the check keeps a wrong caller from injecting script.
pub fn render(reply_to: &ReplyTo, nonce: &str) -> Result<String, PlatformError> {
    let values = [
        ("@SERVICE@", reply_to.service.as_str()),
        ("@PATH@", reply_to.path.as_str()),
        ("@INTERFACE@", reply_to.interface.as_str()),
        ("@NONCE@", nonce),
    ];
    values
        .iter()
        .try_fold(TEMPLATE.to_owned(), |text, (key, value)| {
            if is_plain(value) {
                Ok(text.replace(key, value))
            } else {
                Err(PlatformError::Failed(format!(
                    "refusing {value:?} in the KWin script"
                )))
            }
        })
}

/// The name a query's script is loaded under.
#[must_use]
pub fn name(nonce: &str) -> String {
    format!("{NAME_PREFIX}{nonce}")
}

/// Write `text` as `<dir>/<name>.js`, readable by the user only; creates
/// `dir` when missing.
///
/// # Errors
///
/// When the directory or the file cannot be written.
pub fn write(dir: &Path, name: &str, text: &str) -> Result<PathBuf, PlatformError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .map_err(|error| failed("create", dir, &error))?;
    let path = dir.join(format!("{name}.js"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(&path)
        .map_err(|error| failed("create", &path, &error))?;
    file.write_all(text.as_bytes())
        .map_err(|error| failed("write", &path, &error))?;
    Ok(path)
}

/// Remove a script file written by [`write`]; a missing file is fine.
pub fn remove(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "cannot remove the KWin script");
        }
    }
}

/// Letters, digits and the punctuation of bus names and object paths.
fn is_plain(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '/' | '_' | '-'))
}

fn failed(action: &str, path: &Path, error: &std::io::Error) -> PlatformError {
    PlatformError::Failed(format!("cannot {action} {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply_to() -> ReplyTo {
        ReplyTo {
            service: ":1.42".to_owned(),
            path: "/dev/soldunov/wye".to_owned(),
            interface: "dev.soldunov.wye.KWin1".to_owned(),
        }
    }

    #[test]
    fn rendering_fills_every_placeholder() {
        let text = render(&reply_to(), "00ff").expect("renders");
        for placeholder in ["@SERVICE@", "@PATH@", "@INTERFACE@", "@NONCE@"] {
            assert!(!text.contains(placeholder), "{placeholder} left in {text}");
        }
        assert!(text.contains(r#""00ff","#));
        assert!(text.contains(r#""/dev/soldunov/wye","#));
        assert!(text.contains(r#"":1.42","#));
    }

    #[test]
    fn a_value_that_could_escape_its_string_is_refused() {
        let hostile = ReplyTo {
            service: r#"x"); workspace.activeWindow.closeWindow(); ("#.to_owned(),
            ..reply_to()
        };
        assert!(render(&hostile, "00ff").is_err());
        assert!(render(&reply_to(), "").is_err());
        assert!(render(&reply_to(), "a\"b").is_err());
    }

    #[test]
    fn the_file_is_private_and_removable() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("wye");
        let path = write(&nested, &name("00ff"), "x").expect("written");
        assert_eq!(path, nested.join("wye-query-00ff.js"));
        let mode = fs::metadata(&path).expect("exists").permissions().mode();
        assert_eq!(mode & 0o777, FILE_MODE);
        remove(&path);
        assert!(!path.exists());
        remove(&path);
    }
}
