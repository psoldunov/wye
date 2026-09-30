//! Where scripts live (SCR-08, 12-data-model.md "Storage"): `transform.js`
//! and `rules/<rule-id>.js` next to `config.toml`.

use std::io;
use std::path::{Path, PathBuf};

use wye_api::Error;
use wye_api::actions::ScriptScope;

/// The global script's file name.
const GLOBAL_FILE: &str = "transform.js";
/// The directory of the rules' scripts.
const RULES_DIR: &str = "rules";
/// A script file's extension.
const EXTENSION: &str = "js";
/// Longest rule ID accepted as a file name.
const MAX_ID_LEN: usize = 128;

/// The script files of one configuration directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptFiles {
    dir: PathBuf,
}

impl ScriptFiles {
    /// The scripts next to `config` (`config.toml`).
    pub(crate) fn beside(config: &Path) -> Self {
        let dir = config
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Self { dir }
    }

    /// The scripts whose `rules/` directory is `rules_dir`.
    pub(crate) fn in_rules_dir(rules_dir: &Path) -> Self {
        let dir = rules_dir
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Self { dir }
    }

    /// The directory holding `config.toml` and `transform.js`.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The directory of the rules' scripts.
    pub(crate) fn rules_dir(&self) -> PathBuf {
        self.dir.join(RULES_DIR)
    }

    /// The scripts in `rules/` now.
    pub(crate) fn rule_scripts(&self) -> Vec<ScriptScope> {
        std::fs::read_dir(self.rules_dir())
            .into_iter()
            .flatten()
            .filter_map(|entry| self.scope_of(&entry.ok()?.path()))
            .collect()
    }

    /// The file of `scope`.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` when a rule ID cannot be a file name.
    pub(crate) fn path(&self, scope: &ScriptScope) -> Result<PathBuf, Error> {
        match scope {
            ScriptScope::Global => Ok(self.dir.join(GLOBAL_FILE)),
            ScriptScope::Rule(id) => {
                check_id(id)?;
                Ok(self.dir.join(RULES_DIR).join(format!("{id}.{EXTENSION}")))
            }
        }
    }

    /// The scope a changed file belongs to, if it is a script.
    pub(crate) fn scope_of(&self, path: &Path) -> Option<ScriptScope> {
        let relative = path.strip_prefix(&self.dir).ok()?;
        let parts: Vec<_> = relative.iter().map(|part| part.to_str()).collect();
        match parts.as_slice() {
            [Some(GLOBAL_FILE)] => Some(ScriptScope::Global),
            [Some(RULES_DIR), Some(file)] => {
                let id = file.strip_suffix(&format!(".{EXTENSION}"))?;
                check_id(id).ok()?;
                Some(ScriptScope::Rule(id.to_owned()))
            }
            _ => None,
        }
    }

    /// The text of `scope`'s file; `None` when there is no file.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` for a bad rule ID, `Failed` when the file exists but
    /// cannot be read.
    pub(crate) fn read(&self, scope: &ScriptScope) -> Result<Option<String>, Error> {
        let path = self.path(scope)?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::failed(format!(
                "cannot read {}: {error}",
                path.display()
            ))),
        }
    }

    /// Replace `scope`'s file atomically, keeping its permissions.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` for a bad rule ID, `Failed` when the file cannot be
    /// written (for example a symlink a dotfile manager owns).
    pub(crate) fn write(&self, scope: &ScriptScope, text: &str) -> Result<(), Error> {
        let path = self.path(scope)?;
        wye_desktop::atomic::write(&path, text.as_bytes(), Some(&path))
            .map_err(|error| Error::failed(format!("cannot save {}: {error}", path.display())))
    }
}

/// A rule ID names a file: letters, digits, `-` and `_` only.
fn check_id(id: &str) -> Result<(), Error> {
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(Error::invalid_args(format!(
            "rule ID \u{201c}{id}\u{201d} can only use letters, digits, \u{201c}-\u{201d} and \u{201c}_\u{201d}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> ScriptFiles {
        ScriptFiles::beside(Path::new("/c/wye/config.toml"))
    }

    #[test]
    fn scopes_map_to_files_and_back() {
        let files = files();
        for (scope, path) in [
            (ScriptScope::Global, "/c/wye/transform.js"),
            (ScriptScope::Rule("rule-1".into()), "/c/wye/rules/rule-1.js"),
        ] {
            let mapped = files.path(&scope).expect("valid");
            assert_eq!(mapped, Path::new(path));
            assert_eq!(files.scope_of(&mapped), Some(scope));
        }
    }

    #[test]
    fn other_files_are_not_scripts() {
        let files = files();
        for path in [
            "/c/wye/config.toml",
            "/c/wye/rules/a.txt",
            "/c/wye/rules/sub/a.js",
            "/c/wye/rules/.hidden.js",
            "/elsewhere/transform.js",
        ] {
            assert_eq!(files.scope_of(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn rule_ids_cannot_leave_the_directory() {
        let files = files();
        for id in ["../x", "a/b", "", ".", "a.b", &"x".repeat(MAX_ID_LEN + 1)] {
            assert!(
                matches!(
                    files.path(&ScriptScope::Rule(id.to_owned())),
                    Err(Error::InvalidArgs(_))
                ),
                "{id}"
            );
        }
    }

    #[test]
    fn missing_files_read_as_none_and_writes_are_atomic() {
        let dir = tempfile::tempdir().expect("temp dir");
        let files = ScriptFiles::beside(&dir.path().join("config.toml"));
        let scope = ScriptScope::Rule("r".into());
        assert_eq!(files.read(&scope).expect("read"), None);
        files
            .write(&scope, "export default () => {}")
            .expect("written");
        assert_eq!(
            files.read(&scope).expect("read").as_deref(),
            Some("export default () => {}")
        );
        assert!(dir.path().join("rules/r.js").is_file());
    }
}
