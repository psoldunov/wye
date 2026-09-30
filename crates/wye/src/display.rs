//! Formatting shared by the commands.

use wye_core::{OpenOptions, Target};
use wye_desktop::{Inventory, LaunchCommand};

/// A target as the user knows it: the app's name with the target in
/// parentheses, such as `Firefox (firefox.desktop (private))`, or just the
/// target when the app is unknown.
pub fn target(target: &Target, inventory: &Inventory) -> String {
    match target.desktop_id().and_then(|id| inventory.get(id)) {
        Some(app) => format!("{} ({target})", app.entry.name),
        None => target.to_string(),
    }
}

/// The short name of a target: the app's name, or the target itself.
pub fn target_name(target: &Target, inventory: &Inventory) -> String {
    target
        .desktop_id()
        .and_then(|id| inventory.get(id))
        .map_or_else(|| target.to_string(), |app| app.entry.name.clone())
}

/// `background, new window`, or nothing when neither is set.
pub fn options(options: OpenOptions) -> Option<String> {
    let parts: Vec<&str> = [
        (options.background, "background"),
        (options.new_window, "new window"),
    ]
    .into_iter()
    .filter_map(|(set, label)| set.then_some(label))
    .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// A command line as it could be typed into a POSIX shell.
pub fn command(command: &LaunchCommand) -> String {
    std::iter::once(&command.program)
        .chain(&command.args)
        .map(|word| shell_quote(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quotes `word` for a POSIX shell when it contains anything but safe
/// characters.
fn shell_quote(word: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c);
    if !word.is_empty() && word.chars().all(safe) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("https://a.b/c?d=1"), "'https://a.b/c?d=1'");
        assert_eq!(shell_quote("--new-window"), "--new-window");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn command_line() {
        let command = LaunchCommand {
            program: "/usr/bin/firefox".to_owned(),
            args: vec!["--new-window".to_owned(), "https://x.y/a b".to_owned()],
            remove_env: Vec::new(),
        };
        assert_eq!(
            super::command(&command),
            "/usr/bin/firefox --new-window 'https://x.y/a b'"
        );
    }

    #[test]
    fn option_labels() {
        assert_eq!(options(OpenOptions::default()), None);
        let both = OpenOptions {
            background: true,
            new_window: true,
        };
        assert_eq!(options(both).as_deref(), Some("background, new window"));
    }
}
