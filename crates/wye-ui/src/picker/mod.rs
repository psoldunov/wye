//! The picker's logic on the UI side (02-picker.md, 15-keyboard.md), kept
//! out of the bridge so it is tested without Qt. `bridge/picker.rs`
//! converts types and calls it.
//!
//! - [`view`]: a `PickerRequest` read into tiles, keymap and metrics.
//! - [`state`]: selection, held modifiers and what each input does.
//! - [`keys`]: Qt key events as the core keymap reads them.
//! - [`qml`]: the JSON the QML side draws from.

pub mod keys;
pub mod qml;
pub mod state;
pub mod view;

#[cfg(test)]
mod fixture;
