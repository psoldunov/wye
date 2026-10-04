//! The command line ([11-url-pipeline.md, "Entry points"](../../../docs/spec/11-url-pipeline.md#entry-points),
//! IN-07 and IN-08).

use clap::{Args, Parser, Subcommand, ValueEnum};
use wye_core::{DesktopId, EntryPoint, Force, Modifiers, SourceApp};

/// Wye sends every link to the right browser, profile or app.
#[derive(Debug, Parser)]
#[command(name = "wye", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open links the way Wye would route them (IN-01, IN-07).
    Open(OpenArgs),
    /// Show where a link would open, without opening it (IN-08).
    Test(TestArgs),
    /// List the installed browsers and their profiles.
    Browsers,
    /// Show or change the default web browser (DEF-02, DEF-05).
    Default {
        #[command(subcommand)]
        action: Option<DefaultAction>,
    },
    /// Show the configuration file's path or check it.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Run the Wye session service (DEF-04).
    Service {
        /// Only make sure the service runs, through D-Bus activation, and
        /// exit (for the autostart entry, GEN-01).
        #[arg(long)]
        activate: bool,
    },
    /// Open the URL on the clipboard (IN-02 to IN-04).
    Clipboard {
        /// Open it in the alternative browser (IN-04).
        #[arg(long)]
        alternative: bool,
    },
    /// Open or close the tray menu (TRAY-08).
    Menu,
    /// Open the Settings window (SET-04).
    Settings {
        /// The page to show, such as `browsers` or `rules`.
        #[arg(value_name = "PAGE")]
        page: Option<String>,
    },
    /// Diagnostics for what only a real session can show.
    Debug {
        #[command(subcommand)]
        action: DebugAction,
    },
    /// Connect the browser extension to Wye (BEXT-04). The service writes
    /// the host manifests itself when it starts; these write or remove them
    /// now.
    Extension {
        #[command(subcommand)]
        action: ExtensionAction,
    },
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum ExtensionAction {
    /// Write the native-messaging host manifest for every detected browser,
    /// and let the service keep them current again after `remove`.
    Install,
    /// Delete the manifests and stop the service from writing them again,
    /// until `install`.
    Remove,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum DebugAction {
    /// Print the held modifiers, the pointer and the focused app, after an
    /// optional delay to set them up.
    Probe {
        /// Seconds to wait first.
        #[arg(long, value_name = "SECONDS")]
        delay: Option<u64>,
    },
}

/// Overrides of the normal decision, shared by `open` and `test`.
#[derive(Debug, Args)]
pub struct ForceArgs {
    /// Always ask with the picker (PIPE-11).
    #[arg(long, conflicts_with = "alternative")]
    pub pick: bool,
    /// Open in the alternative browser, as if its key were held (PIPE-06).
    #[arg(long)]
    pub alternative: bool,
}

impl ForceArgs {
    pub fn force(&self) -> Force {
        if self.pick {
            Force::Picker
        } else if self.alternative {
            Force::Alternative
        } else {
            Force::None
        }
    }
}

#[derive(Debug, Args)]
pub struct OpenArgs {
    #[command(flatten)]
    pub force: ForceArgs,
    /// Links to open.
    #[arg(value_name = "URL")]
    pub urls: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TestArgs {
    /// The link to test.
    #[arg(value_name = "URL")]
    pub url: String,
    /// The app the link came from: a desktop ID or an executable name.
    #[arg(long, value_name = "APP", value_parser = parse_source)]
    pub source: Option<SourceApp>,
    /// Modifier keys held while clicking, such as Shift+Ctrl.
    #[arg(long, value_name = "KEYS", value_parser = parse_keys)]
    pub keys: Option<Modifiers>,
    /// How the link reached Wye.
    #[arg(long, value_enum, default_value_t = Entry::Handler)]
    pub entry: Entry,
    /// Pretend the screen is locked.
    #[arg(long)]
    pub locked: bool,
    #[command(flatten)]
    pub force: ForceArgs,
}

/// [`EntryPoint`] as a command-line value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Entry {
    Handler,
    Clipboard,
    Extension,
    Cli,
}

impl From<Entry> for EntryPoint {
    fn from(entry: Entry) -> Self {
        match entry {
            Entry::Handler => Self::Handler,
            Entry::Clipboard => Self::Clipboard,
            Entry::Extension => Self::Extension,
            Entry::Cli => Self::Cli,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Subcommand)]
pub enum DefaultAction {
    /// Show the current default browser.
    #[default]
    Status,
    /// Make Wye the default browser, remembering the current one.
    Set,
    /// Give the default back to the browser Wye replaced. Does nothing
    /// when Wye is no longer the default.
    Unset,
}

#[derive(Debug, Clone, Copy, Default, Subcommand)]
pub enum ConfigAction {
    /// Print the configuration file's path.
    #[default]
    Path,
    /// Report problems in the configuration file.
    Check,
}

/// A value ending in `.desktop` is a desktop ID; anything else is an
/// executable name (SRC-01).
fn parse_source(value: &str) -> Result<SourceApp, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("expected a desktop ID or an executable name".to_owned());
    }
    if value.ends_with(DesktopId::SUFFIX) {
        let id = DesktopId::new(value).map_err(|error| error.to_string())?;
        Ok(SourceApp {
            desktop_id: Some(id),
            executable: None,
        })
    } else {
        Ok(SourceApp {
            desktop_id: None,
            executable: Some(value.to_owned()),
        })
    }
}

fn parse_keys(value: &str) -> Result<Modifiers, String> {
    value
        .parse()
        .map_err(|error: wye_core::keys::UnknownModifier| error.to_string())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn source_desktop_id() {
        let source = parse_source("com.slack.Slack.desktop").unwrap();
        assert_eq!(
            source.desktop_id.unwrap().as_str(),
            "com.slack.Slack.desktop"
        );
        assert_eq!(source.executable, None);
    }

    #[test]
    fn source_executable() {
        let source = parse_source("slack").unwrap();
        assert_eq!(source.desktop_id, None);
        assert_eq!(source.executable.as_deref(), Some("slack"));
    }

    #[test]
    fn source_rejects_bad_values() {
        assert!(parse_source("  ").is_err());
        assert!(parse_source("a b.desktop").is_err());
    }

    #[test]
    fn keys() {
        assert_eq!(
            parse_keys("Shift+Ctrl").unwrap(),
            "ctrl,shift".parse().unwrap()
        );
        assert!(parse_keys("Hyper").is_err());
    }

    #[test]
    fn pick_conflicts_with_alternative() {
        let result = Cli::try_parse_from(["wye", "open", "--pick", "--alternative", "x"]);
        assert!(result.is_err());
    }

    #[test]
    fn extension_takes_install_or_remove_bext_04() {
        let action = |args: &[&str]| match Cli::try_parse_from(args).map(|cli| cli.command) {
            Ok(Command::Extension { action }) => Some(action),
            _ => None,
        };
        assert!(matches!(
            action(&["wye", "extension", "install"]),
            Some(ExtensionAction::Install)
        ));
        assert!(matches!(
            action(&["wye", "extension", "remove"]),
            Some(ExtensionAction::Remove)
        ));
        assert!(action(&["wye", "extension"]).is_none());
    }
}
