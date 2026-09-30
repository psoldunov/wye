//! Wye's internal state (`$XDG_STATE_HOME/wye/state.toml`). The type lives
//! in `wye-desktop` so the CLI and the service share one file format.

pub use wye_desktop::state::State;
