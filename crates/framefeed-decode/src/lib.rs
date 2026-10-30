//! Decoder backends.
//!
//! M1 uses the **ffmpeg sidecar**: a child process decodes and (via swscale)
//! scales to the exact raster we ask for, and we read raw RGBA frames from its
//! stdout. That keeps the install dependency to "have ffmpeg on PATH" and avoids
//! FFmpeg dev libraries on every platform. The [`Decoder`] trait is the seam for
//! a future in-process `libav` backend (see `docs/adr/0002-ffmpeg-sidecar.md`).
//!
//! The trait hands out a borrow of an internal raster rather than a fresh
//! `Frame`, so the steady-state loop performs no per-frame allocations.

#![forbid(unsafe_code)]

mod ffmpeg;

pub use ffmpeg::{DecodeError, FfmpegDecoder, OutputSize, VideoMetadata, probe};

use framefeed_core::Raster;

/// A video source that yields RGB24 rasters sized for the terminal.
pub trait Decoder {
    /// Source metadata, available before the first frame.
    fn metadata(&self) -> &VideoMetadata;

    /// Decodes the next frame into the internal raster.
    ///
    /// Returns `Ok(None)` at end of stream. The borrow ends when the caller drops
    /// it, so callers must finish encoding before calling this again.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when the decoder fails or the stream is truncated.
    fn next_frame(&mut self) -> Result<Option<&Raster>, DecodeError>;
}
