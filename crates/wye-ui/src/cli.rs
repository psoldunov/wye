//! Command line of `wye-ui`.

use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use wye_api::actions::Window;

use crate::surface::Surface;

/// Wye's UI host: the picker, Settings and the other windows.
///
/// Normally started by D-Bus activation and left running. Starting it again
/// hands WINDOW to the running instance and exits.
#[derive(Debug, Parser)]
#[command(name = "wye-ui", version)]
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

    /// Load every surface (or SURFACE) offscreen with its fixtures and fail
    /// on any QML warning or error.
    #[arg(
        long,
        value_name = "SURFACE",
        num_args = 0..=1,
        default_missing_value = "all",
        conflicts_with_all = ["window", "self_test_child"],
    )]
    pub self_test: Option<String>,

    /// Internal: one self-test surface, in this process.
    #[arg(long = "self-test-child", value_name = "SURFACE", hide = true, value_parser = parse_surface)]
    pub self_test_child: Option<Surface>,

    /// With --self-test: deliver the cases one at a time and save a PNG of
    /// every visible window after each into DIR (a dev tool; Qt warnings
    /// are ignored).
    #[arg(long, value_name = "DIR", requires = "self_tests")]
    pub snapshots: Option<PathBuf>,
}

fn parse_window(text: &str) -> Result<Window, String> {
    text.parse()
        .map_err(|error: wye_api::UnknownValue| error.to_string())
}

fn parse_surface(text: &str) -> Result<Surface, String> {
    Surface::from_name(text).ok_or_else(|| format!("unknown surface {text:?}"))
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
        let cli = Cli::try_parse_from(["wye-ui", "settings", "rules"]).expect("parses");
        assert_eq!(cli.window, Some(Window::Settings));
        assert_eq!(cli.argument, "rules");
        assert!(Cli::try_parse_from(["wye-ui", "nope"]).is_err());
    }

    #[test]
    fn self_test_defaults_to_every_surface() {
        let cli = Cli::try_parse_from(["wye-ui", "--self-test"]).expect("parses");
        assert_eq!(cli.self_test.as_deref(), Some("all"));
        let cli = Cli::try_parse_from(["wye-ui", "--self-test", "picker"]).expect("parses");
        assert_eq!(cli.self_test.as_deref(), Some("picker"));
    }

    #[test]
    fn snapshots_need_the_self_test() {
        let cli = Cli::try_parse_from(["wye-ui", "--self-test", "--snapshots", "/tmp/s"])
            .expect("parses");
        assert_eq!(cli.self_test.as_deref(), Some("all"));
        assert_eq!(cli.snapshots, Some(PathBuf::from("/tmp/s")));
        let cli = Cli::try_parse_from(["wye-ui", "--self-test", "about", "--snapshots", "s"])
            .expect("parses");
        assert_eq!(cli.self_test.as_deref(), Some("about"));
        assert!(Cli::try_parse_from(["wye-ui", "--snapshots", "s"]).is_err());
        assert!(Cli::try_parse_from(["wye-ui", "settings", "--snapshots", "s"]).is_err());
    }

    #[test]
    fn the_child_flag_takes_a_surface() {
        let cli = Cli::try_parse_from(["wye-ui", CHILD_FLAG, "tray-menu"]).expect("parses");
        assert_eq!(cli.self_test_child, Some(Surface::TrayMenu));
    }
}
