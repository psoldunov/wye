//! Replacing files atomically.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to `path` so readers see either the old or the new file,
/// never a half-written one: the bytes go to a fresh temporary file in the
/// same directory, which is synced and renamed over `path`. Missing parent
/// directories are created.
///
/// The new file has the permissions of `keep_permissions_from` when that is
/// an existing regular file, else the temporary file's owner-only `0600`.
///
/// A symlink at `path` is never followed or replaced: whatever manages the
/// link owns the file behind it, so this returns an error instead.
///
/// # Errors
///
/// Returns the error from creating, writing, syncing or renaming the file,
/// and [`io::ErrorKind::InvalidInput`] when `path` is a symlink.
pub fn write(path: &Path, bytes: &[u8], keep_permissions_from: Option<&Path>) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is a symlink; not replacing it", path.display()),
        ));
    }
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    let permissions = keep_permissions_from
        .and_then(|source| fs::symlink_metadata(source).ok())
        .filter(fs::Metadata::is_file)
        .map(|meta| meta.permissions());
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    // On error the temporary file is removed when `temp` is dropped.
    temp.persist(path).map_err(|error| error.error)?;
    // Best effort: syncing the directory makes the rename durable, but the
    // file is already in place and some filesystems refuse to sync a
    // directory.
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn creates_directories_and_leaves_no_temp_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a/b/file");
        write(&path, b"one", None).unwrap();
        write(&path, b"two", None).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(names(&root.path().join("a/b")), vec!["file"]);
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn keeps_permissions_from_a_regular_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write(&path, b"new", Some(&path)).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[test]
    fn refuses_a_symlink_at_the_destination() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        fs::write(&target, "kept").unwrap();
        let link = root.path().join("link");
        symlink(&target, &link).unwrap();
        let error = write(&link, b"new", Some(&link)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(names(root.path()), vec!["link", "target"]);
    }
}
