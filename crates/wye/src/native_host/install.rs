//! Writing and deleting the host manifests of every detected browser
//! (BEXT-04), through `wye_desktop::native_messaging`: `wye extension
//! install|remove` and `wye-native-host --install|--remove` both run this
//! file (each binary includes it).

use std::io::{self, Write};
use std::path::Path;

use wye_desktop::native_messaging::{self, Manifest};
use wye_desktop::xdg::XdgDirs;

const UNSUPPORTED: &str =
    "Flatpak and Snap browsers are not supported: their sandbox cannot start the host.";

/// Write a manifest naming the host for every browser found and list them
/// on `out`.
///
/// # Errors
///
/// When the host cannot be found or a manifest cannot be written.
pub fn install(
    out: &mut impl Write,
    xdg: &XdgDirs,
    current_exe: Option<&Path>,
) -> Result<(), String> {
    let host = native_messaging::find_host(xdg, current_exe).ok_or_else(|| {
        format!(
            "cannot find {} on PATH or next to this program",
            native_messaging::HOST_PROGRAM
        )
    })?;
    let written = native_messaging::install(xdg, &host).map_err(|error| error.to_string())?;
    report(
        out,
        "Installed",
        &written,
        "No supported browser was found.",
    )
    .map_err(|error| io_error(&error))?;
    writeln!(out, "{UNSUPPORTED}").map_err(|error| io_error(&error))
}

/// Delete every manifest and list them on `out`.
///
/// # Errors
///
/// When a manifest cannot be deleted.
pub fn remove(out: &mut impl Write, xdg: &XdgDirs) -> Result<(), String> {
    let removed = native_messaging::remove(xdg).map_err(|error| error.to_string())?;
    report(out, "Removed", &removed, "No manifest was installed.").map_err(|error| io_error(&error))
}

fn report(out: &mut impl Write, verb: &str, manifests: &[Manifest], none: &str) -> io::Result<()> {
    if manifests.is_empty() {
        return writeln!(out, "{none}");
    }
    manifests.iter().try_for_each(|manifest| {
        writeln!(
            out,
            "{verb} for {}: {}",
            manifest.browser,
            manifest.path.display()
        )
    })
}

fn io_error(error: &io::Error) -> String {
    format!("cannot write the report: {error}")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use wye_desktop::xdg::Locale;

    use super::*;

    fn xdg(root: &Path) -> XdgDirs {
        let home = root.join("home");
        XdgDirs {
            config_home: home.join(".config"),
            config_dirs: Vec::new(),
            data_home: home.join(".local/share"),
            data_dirs: Vec::new(),
            current_desktops: Vec::new(),
            search_path: vec![root.join("bin")],
            locale: Locale::none(),
            home,
        }
    }

    #[test]
    fn install_and_remove_report_each_browser_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        fs::create_dir_all(xdg.home.join(".mozilla/firefox")).expect("firefox");
        fs::create_dir_all(root.path().join("bin")).expect("bin");
        fs::write(root.path().join("bin/wye-native-host"), "").expect("host");

        let mut out = Vec::new();
        install(&mut out, &xdg, None).expect("installed");
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.starts_with("Installed for Firefox: "), "{text}");
        assert!(text.contains("Flatpak"));
        let manifest = xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json");
        let written = fs::read_to_string(&manifest).expect("manifest");
        assert!(
            written.contains(
                &root
                    .path()
                    .join("bin/wye-native-host")
                    .display()
                    .to_string()
            )
        );

        let mut out = Vec::new();
        remove(&mut out, &xdg).expect("removed");
        assert!(
            String::from_utf8(out)
                .expect("UTF-8")
                .starts_with("Removed for Firefox")
        );
        assert!(!manifest.exists());
    }

    #[test]
    fn without_the_host_nothing_is_written() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        let error = install(&mut Vec::new(), &xdg, None).expect_err("no host");
        assert!(error.contains("wye-native-host"));
    }
}
