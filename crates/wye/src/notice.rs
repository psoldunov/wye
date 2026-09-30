//! Diagnostics that must never stop a link opening.
//!
//! Links clicked in apps often reach Wye with a closed, full or otherwise
//! broken stderr. Warnings and notices on the routing path are written best
//! effort, so a failed write loses the message rather than the link.

use std::fmt;
use std::io::Write;

/// Writes `message` and a newline to `err`, ignoring write errors.
pub fn write(err: &mut dyn Write, message: fmt::Arguments<'_>) {
    // Nothing useful can be done when stderr fails: the message is lost.
    writeln!(err, "{message}").ok();
}

#[cfg(test)]
pub mod testing {
    use std::io;

    /// A writer whose every write fails, like a full or closed stderr.
    pub struct Broken;

    impl io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Broken;
    use super::*;

    #[test]
    fn writes_a_line() {
        let mut out = Vec::new();
        write(&mut out, format_args!("wye: {}", 1));
        assert_eq!(out, b"wye: 1\n");
    }

    #[test]
    fn ignores_a_broken_writer() {
        write(&mut Broken, format_args!("lost"));
    }
}
