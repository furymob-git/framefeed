//! Terminal display protocols: capability detection and byte-exact encoders.
//!
//! Encoders are pure functions over buffers: no terminal, no clock, no I/O. That
//! makes every escape sequence testable as bytes in CI, and keeps performance
//! policy (raster caps, fps) in [`Budget`] rather than scattered through the
//! protocol code.

#![forbid(unsafe_code)]

mod ansi;
mod budget;
mod detect;
mod kitty;
mod sixel;
mod terminal;

pub use ansi::AnsiEncoder;
pub use budget::Budget;
pub use detect::{Backend, Detection, Hints, Multiplexer, detect};
pub use kitty::{Compression, EncodeError, KittyEncoder, KittyOptions};
pub use sixel::SixelEncoder;
pub use terminal::{
    TerminalGuard, WT_SIXEL_CELL, stdout_is_tty, viewport, viewport_for, viewport_from_env,
};
