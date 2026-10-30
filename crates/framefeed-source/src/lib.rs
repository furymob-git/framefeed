//! Source resolution: the `--quality` model and HLS playlist handling.
//!
//! Keeping variant selection here (rather than delegating it to the decoder) is
//! what makes `--quality 144p` actually save bandwidth: we choose a variant from
//! the master playlist *before* fetching any video. See `docs/adr/0004-quality-model.md`.

#![forbid(unsafe_code)]

pub mod hls;
pub mod quality;

pub use hls::{
    HlsError, MasterPlaylist, Variant, parse_master_playlist, resolve_uri, select_variant,
};
pub use quality::{Quality, QualityError};
