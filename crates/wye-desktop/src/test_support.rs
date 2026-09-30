//! Temporary XDG trees for tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::xdg::XdgDirs;

pub struct Fixture {
    pub root: TempDir,
    pub xdg: XdgDirs,
}

impl Fixture {
    /// A home under a temp dir with one system data dir (`sys`) and a `bin`
    /// directory on the search path.
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let xdg = XdgDirs {
            config_home: home.join(".config"),
            config_dirs: vec![root.path().join("etc/xdg")],
            data_home: home.join(".local/share"),
            data_dirs: vec![root.path().join("sys")],
            current_desktops: vec!["GNOME".into()],
            search_path: vec![root.path().join("bin")],
            home,
        };
        fs::create_dir_all(&xdg.home).unwrap();
        fs::create_dir_all(root.path().join("bin")).unwrap();
        Self { root, xdg }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.path().join(rel)
    }

    /// Writes `text` to `rel` under the fixture root, creating parents.
    pub fn write(&self, rel: &str, text: &str) -> PathBuf {
        write_file(&self.path(rel), text)
    }

    /// Writes an entry into the user's applications dir.
    pub fn user_entry(&self, name: &str, text: &str) -> PathBuf {
        write_file(&self.xdg.data_home.join("applications").join(name), text)
    }

    /// Writes an entry into the system applications dir.
    pub fn system_entry(&self, name: &str, text: &str) -> PathBuf {
        self.write(&format!("sys/applications/{name}"), text)
    }

    /// Creates an executable in the fixture's `bin` dir.
    pub fn program(&self, name: &str) -> PathBuf {
        let path = self.write(&format!("bin/{name}"), "#!/bin/sh\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

pub fn write_file(path: &Path, text: &str) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    path.to_path_buf()
}

pub const FIREFOX: &str = "[Desktop Entry]
Name=Firefox
Icon=firefox
Type=Application
Exec=firefox --name firefox %u
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
Actions=new-window;new-private-window;

[Desktop Action new-window]
Name=New Window
Exec=firefox --new-window %u

[Desktop Action new-private-window]
Name=New Private Window
Exec=firefox --private-window %u
";

pub const CHROME: &str = "[Desktop Entry]
Name=Google Chrome
Type=Application
Exec=/usr/bin/google-chrome-stable %U
MimeType=x-scheme-handler/http;x-scheme-handler/https;
";

pub const EDITOR: &str = "[Desktop Entry]
Name=Editor
Type=Application
Exec=editor %F
MimeType=text/plain;
";
