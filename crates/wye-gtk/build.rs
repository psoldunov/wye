//! Build script of `wye-gtk`:
//!
//! - `data/resources.gresource.xml`: the `GResource` (CSS, icons, the app
//!   icon), compiled with `glib-compile-resources` and registered at
//!   start-up (`crate::app`).
//! - `fixtures/*.json`: embedded for `wye-gtk --self-test`
//!   (`$OUT_DIR/fixtures.rs`), so the installed binary carries them.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the shipped app icon lives; the `GResource` bundles it so the About
/// dialog shows it without an installed icon theme.
const APP_ICON_DIR: &str = "../../data/icons/hicolor/scalable/apps";

fn main() {
    glib_build_tools::compile_resources(
        &["data", APP_ICON_DIR],
        "data/resources.gresource.xml",
        "wye-gtk.gresource",
    );
    println!("cargo::rerun-if-changed={APP_ICON_DIR}/dev.soldunov.wye.svg");
    // A directory makes Cargo rerun the script when anything inside changes,
    // new files included.
    println!("cargo::rerun-if-changed=fixtures");
    if let Err(error) = write_fixtures() {
        // Cargo shows `cargo::error` as the build's diagnostic; the exit
        // status fails the build.
        println!("cargo::error=wye-gtk build script: {error}");
        std::process::exit(1);
    }
}

/// `$OUT_DIR/fixtures.rs`: `FIXTURES`, each file stem with its text.
fn write_fixtures() -> Result<(), String> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let entries: String = json_files(&manifest.join("fixtures"))?
        .iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            Some(format!(
                "    ({stem:?}, include_str!({:?})),\n",
                path.display().to_string()
            ))
        })
        .collect();
    let source = format!(
        "/// Self-test fixtures: file stem and JSON text.\npub const FIXTURES: &[(&str, &str)] = &[\n{entries}];\n"
    );
    let out = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    fs::write(out.join("fixtures.rs"), source)
        .map_err(|error| format!("cannot write fixtures.rs: {error}"))
}

/// The `*.json` files directly in `dir`, sorted.
fn json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
