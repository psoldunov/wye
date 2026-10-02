//! Command line of `wye-gtk`.

use std::path::PathBuf;

use clap::{ArgGroup, Parser, ValueEnum};
use wye_api::actions::Window;

use crate::surface::Surface;

/// Wye's GTK window host for GNOME: Settings and the other windows.
///
/// Normally started by D-Bus activation and left running. Starting it again
/// hands WINDOW to the running instance and exits.
#[derive(Debug, Parser)]
#[command(name = "wye-gtk", version)]
#[command(group = ArgGroup::new("self_tests").args(["self_test", "self_test_child"]))]
pub struct Cli {
    /// Window to open: settings, first-run, history, test-rules, about,
    /// script-editor or rule-editor.
    #[arg(value_parser = parse_window)]
    pub window: Option<Window>,

    /// The window's argument: a Settings page, a script scope or a rule
    /// editor prefill (JSON).
    #[arg(requires = "window", default_value = "")]
    pub argument: String,

    /// Show every surface (or SURFACE) on a private display with its
    /// fixtures and fail on any GTK or `GLib` warning or critical.
    #[arg(
        long,
        value_name = "SURFACE",
        num_args = 0..=1,
        value_parser = parse_surface,
        conflicts_with_all = ["window", "self_test_child"],
    )]
    #[allow(
        clippy::option_option,
        reason = "clap's own shape for a flag with an optional value: `--self-test` alone is every surface"
    )]
    pub self_test: Option<Option<Surface>>,

    /// Internal: one self-test surface, in this process.
    #[arg(long = "self-test-child", value_name = "SURFACE", hide = true, value_parser = parse_surface)]
    pub self_test_child: Option<Surface>,

    /// With --self-test: after each case, save a PNG of every visible window
    /// into DIR (a dev tool; warnings are still reported).
    #[arg(long, value_name = "DIR", requires = "self_tests")]
    pub snapshots: Option<PathBuf>,

    /// With --self-test: force the light or dark style for every case (a
    /// case's own `scheme` still wins).
    #[arg(long, value_enum, requires = "self_tests")]
    pub scheme: Option<Scheme>,

    /// With --snapshots: the scale factor to render at (`GDK_SCALE`), for
    /// crisp images. Defaults to `GDK_SCALE`, else 1.
    #[arg(long, value_name = "FACTOR", requires = "snapshots", value_parser = clap::value_parser!(u8).range(1..=4))]
    pub scale: Option<u8>,
}

/// A colour scheme the self-test can force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    /// The name a fixture's `scheme` uses.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Parse a fixture's `scheme`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

fn parse_window(text: &str) -> Result<Window, String> {
    text.parse()
        .map_err(|error: wye_api::UnknownValue| error.to_string())
}

/// A surface by name; the error lists the names there are.
fn parse_surface(text: &str) -> Result<Surface, String> {
    Surface::from_name(text).ok_or_else(|| {
        let names = Surface::ALL.map(Surface::name).join(", ");
        format!("unknown surface {text:?}; one of {names}")
    })
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    use super::*;
    use crate::selftest::CHILD_FLAG;

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_window_and_its_argument_parse() {
        let cli = Cli::try_parse_from(["wye-gtk", "settings", "rules"]).expect("parses");
        assert_eq!(cli.window, Some(Window::Settings));
        assert_eq!(cli.argument, "rules");
        assert!(Cli::try_parse_from(["wye-gtk", "nope"]).is_err());
    }

    #[test]
    fn self_test_takes_one_surface_or_every_one() {
        let cli = Cli::try_parse_from(["wye-gtk", "--self-test"]).expect("parses");
        assert_eq!(cli.self_test, Some(None), "every surface");
        let cli = Cli::try_parse_from(["wye-gtk", "--self-test", "about"]).expect("parses");
        assert_eq!(cli.self_test, Some(Some(Surface::About)));
        let error = Cli::try_parse_from(["wye-gtk", "--self-test", "tray"]).expect_err("unknown");
        assert!(error.to_string().contains("tray-menu"), "{error}");
    }

    #[test]
    fn snapshots_scheme_and_scale_need_the_self_test() {
        let cli = Cli::try_parse_from([
            "wye-gtk",
            "--self-test",
            "settings",
            "--snapshots",
            "/tmp/s",
            "--scheme",
            "dark",
            "--scale",
            "2",
        ])
        .expect("parses");
        assert_eq!(cli.snapshots, Some(PathBuf::from("/tmp/s")));
        assert_eq!(cli.scheme, Some(Scheme::Dark));
        assert_eq!(cli.scale, Some(2));
        assert!(Cli::try_parse_from(["wye-gtk", "--snapshots", "s"]).is_err());
        assert!(Cli::try_parse_from(["wye-gtk", "--scheme", "dark"]).is_err());
        assert!(Cli::try_parse_from(["wye-gtk", "--self-test", "--scale", "2"]).is_err());
        assert!(
            Cli::try_parse_from(["wye-gtk", "--self-test", "--snapshots", "s", "--scale", "9"])
                .is_err()
        );
    }

    #[test]
    fn the_child_flag_takes_a_surface() {
        let cli = Cli::try_parse_from(["wye-gtk", CHILD_FLAG, "about"]).expect("parses");
        assert_eq!(cli.self_test_child, Some(Surface::About));
    }

    #[test]
    fn schemes_round_trip() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            assert_eq!(Scheme::parse(scheme.as_str()), Some(scheme));
        }
        assert_eq!(Scheme::parse("sepia"), None);
    }
}
