//! What one script run may use (SCR-21).

use std::time::Duration;

/// Run time per call (SCR-21).
pub const TIME_LIMIT: Duration = Duration::from_millis(50);
/// Memory per call (SCR-21), including the engine's own few hundred KB.
pub const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
/// Native stack for the interpreter, so deep recursion fails as a
/// `RangeError` long before the thread's stack runs out.
pub const STACK_LIMIT: usize = 512 * 1024;
/// `console.log` lines kept per run; later lines are counted, not kept.
pub const MAX_LOG_LINES: usize = 100;
/// Characters kept per `console.log` line.
pub const MAX_LOG_CHARS: usize = 2000;

/// The limits of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub time: Duration,
    /// Bytes.
    pub memory: usize,
    /// Bytes.
    pub stack: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            time: TIME_LIMIT,
            memory: MEMORY_LIMIT,
            stack: STACK_LIMIT,
        }
    }
}
