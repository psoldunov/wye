//! Import and export files (RUL-02): the path behind a file dialog's URL,
//! and reading and writing the rules file.

use std::path::PathBuf;

use url::Url;

/// The local path a file dialog chose (`file:///…`), or a plain path.
///
/// # Errors
///
/// When the URL is not a local file.
pub fn local_path(chosen: &str) -> Result<PathBuf, String> {
    if chosen.starts_with('/') {
        return Ok(PathBuf::from(chosen));
    }
    Url::parse(chosen)
        .ok()
        .filter(|url| url.scheme() == "file")
        .and_then(|url| url.to_file_path().ok())
        .ok_or_else(|| format!("{chosen} is not a local file"))
}

/// Write `text` to the file a dialog chose.
///
/// # Errors
///
/// A message for the user.
pub fn write(chosen: &str, text: &str) -> Result<(), String> {
    let path = local_path(chosen)?;
    std::fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Read the file a dialog chose.
///
/// # Errors
///
/// A message for the user.
pub fn read(chosen: &str) -> Result<String, String> {
    let path = local_path(chosen)?;
    std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rul_02_dialog_urls_become_paths() {
        assert_eq!(
            local_path("file:///tmp/a%20b.toml").expect("local"),
            PathBuf::from("/tmp/a b.toml")
        );
        assert_eq!(
            local_path("/tmp/x").expect("local"),
            PathBuf::from("/tmp/x")
        );
        assert!(local_path("https://example.com/x").is_err());
    }

    #[test]
    fn rul_02_files_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("wye-rules.toml");
        let url = Url::from_file_path(&path).expect("url").to_string();
        write(&url, "format = \"wye-rules\"\n").expect("written");
        assert_eq!(read(&url).expect("read"), "format = \"wye-rules\"\n");
        assert!(read("file:///nonexistent/x.toml").is_err());
    }
}
