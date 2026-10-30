//! Per-backend performance budgets.
//!
//! Values come from the targets in `docs/PERFORMANCE.md`. They are policy, not
//! protocol facts: the pipeline treats them as upper bounds for raster height
//! and presentation rate unless the user overrides them explicitly.

/// Resource envelope for a display backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Maximum raster height in pixels before the encoder becomes too expensive.
    pub max_raster_height: u32,
    /// Default presentation rate cap for this backend.
    pub max_fps: u8,
}

impl Budget {
    /// Kitty graphics with shared-memory or compressed transport.
    pub const KITTY: Self = Self {
        max_raster_height: 720,
        max_fps: 60,
    };
    /// Sixel: palette indexing plus a full repaint every frame.
    pub const SIXEL: Self = Self {
        max_raster_height: 480,
        max_fps: 30,
    };
    /// iTerm2 inline images: a full base64 image per frame.
    pub const ITERM2: Self = Self {
        max_raster_height: 480,
        max_fps: 30,
    };
    /// ANSI half-blocks: cost is dominated by SGR sequences.
    pub const ANSI: Self = Self {
        max_raster_height: 384,
        max_fps: 30,
    };
}
