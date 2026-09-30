//! What the About window says about Wye itself (DLG-ABT-01): the object
//! `FormCard.AboutPage` reads as its `aboutData`, in the shape of
//! `KAboutData`.

use serde_json::{Value, json};

/// The project's home, also its issue tracker's parent (Cargo.toml
/// `repository`).
pub const HOMEPAGE: &str = "https://github.com/psoldunov/wye";
/// Where bugs go (DLG-ABT-01, "issue tracker").
pub const ISSUE_TRACKER: &str = "https://github.com/psoldunov/wye/issues";
/// The desktop entry's ID, which is also the application's icon name.
pub const DESKTOP_FILE: &str = "dev.soldunov.wye";
/// What Wye is, in one line.
pub const DESCRIPTION: &str = "Opens every link in the browser you want.";
/// Who wrote it, and the year (LICENSE).
pub const COPYRIGHT: &str = "© 2026 Philipp Soldunov";

const LICENCE_NAME: &str = "MIT License";
const LICENCE_SPDX: &str = "MIT";
/// The text of `LICENSE`. Kept here because the Nix build's source set does
/// not carry the file, so a test cannot compare them; keep both in step.
const LICENCE_TEXT: &str = "MIT License\n\nCopyright (c) 2026 Philipp Soldunov\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n";

/// The UI host's own version, for when the service does not answer.
pub const UI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `aboutData` of the page for `version`, the service's `Version`.
#[must_use]
pub fn about_data(version: &str) -> Value {
    json!({
        "displayName": "Wye",
        "productName": "wye",
        "componentName": "wye",
        "shortDescription": DESCRIPTION,
        "homepage": HOMEPAGE,
        "bugAddress": ISSUE_TRACKER,
        "version": version,
        "otherText": "",
        "authors": [{
            "name": "Philipp Soldunov",
            "task": "Author",
            "emailAddress": "",
            "webAddress": "https://github.com/psoldunov",
            "ocsUsername": ""
        }],
        "credits": [
            {
                "name": "KDE Frameworks and Kirigami",
                "task": "User interface toolkit",
                "emailAddress": "",
                "webAddress": "https://kde.org/products/frameworks/",
                "ocsUsername": ""
            },
            {
                "name": "Qt",
                "task": "Application framework",
                "emailAddress": "",
                "webAddress": "https://www.qt.io/",
                "ocsUsername": ""
            }
        ],
        "translators": [],
        "licenses": [{
            "name": LICENCE_NAME,
            "text": LICENCE_TEXT,
            "spdx": LICENCE_SPDX
        }],
        "releases": [],
        "copyrightStatement": COPYRIGHT,
        "desktopFileName": DESKTOP_FILE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_every_field_it_reads() {
        // DLG-ABT-01: name, icon, version, website, issue tracker, licence, credits
        let data = about_data("1.2.3");
        for key in [
            "displayName",
            "version",
            "homepage",
            "bugAddress",
            "licenses",
            "authors",
            "credits",
            "copyrightStatement",
            "desktopFileName",
            "otherText",
            "translators",
            "releases",
        ] {
            assert!(data.get(key).is_some(), "{key}");
        }
        assert_eq!(data["version"], "1.2.3");
        assert_eq!(data["licenses"][0]["spdx"], "MIT");
    }

    #[test]
    fn the_links_are_the_repository() {
        let manifest = include_str!("../../../../Cargo.toml");
        assert!(manifest.contains(&format!("repository = \"{HOMEPAGE}\"")));
        assert!(ISSUE_TRACKER.starts_with(HOMEPAGE));
    }
}
