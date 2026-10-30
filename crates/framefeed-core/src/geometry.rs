//! Terminal geometry and render planning.
//!
//! The planner decides *how many pixels* we render, never how they are written.
//! It is pure so it can be tested against the awkward cases (tiny terminals,
//! absurd source sizes, backend height caps) without a terminal.

/// Conservative cell size used when the terminal reports no pixel geometry.
///
/// 8×16 is the classic VGA text cell and matches most monospace fonts at
/// default settings; `framefeed doctor` prints when this assumption is in use.
pub const DEFAULT_CELL: CellSize = CellSize {
    width_px: 8,
    height_px: 16,
};

/// Size of one character cell in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSize {
    /// Cell width in pixels.
    pub width_px: u16,
    /// Cell height in pixels.
    pub height_px: u16,
}

impl CellSize {
    /// Creates a cell size, rejecting zero dimensions.
    #[must_use]
    pub fn new(width_px: u16, height_px: u16) -> Option<Self> {
        (width_px > 0 && height_px > 0).then_some(Self {
            width_px,
            height_px,
        })
    }
}

/// A width/height pair in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// Creates a size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Width divided by height, or `None` for a zero-height size.
    #[must_use]
    pub fn aspect_ratio(self) -> Option<f64> {
        (self.height > 0).then(|| f64::from(self.width) / f64::from(self.height))
    }
}

/// The terminal area we may draw into, in cells and in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// Number of columns (characters) available.
    pub cols: u16,
    /// Number of rows (lines) available.
    pub rows: u16,
    /// Pixel size of one cell.
    pub cell: CellSize,
}

impl Viewport {
    /// Creates a viewport, rejecting zero columns or rows.
    #[must_use]
    pub fn new(cols: u16, rows: u16, cell: CellSize) -> Option<Self> {
        (cols > 0 && rows > 0).then_some(Self { cols, rows, cell })
    }

    /// Full drawable area in pixels.
    #[must_use]
    pub fn pixel_size(self) -> Size {
        Size {
            width: u32::from(self.cols) * u32::from(self.cell.width_px),
            height: u32::from(self.rows) * u32::from(self.cell.height_px),
        }
    }
}

/// How the source image is mapped into the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Whole image visible, letterboxed/pillarboxed (default).
    Contain,
    /// Viewport fully covered, overflowing source edges cropped.
    Cover,
    /// Whole image visible, aspect ratio ignored.
    Stretch,
}

/// Region of the source image to keep, in source pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    /// Left edge in source pixels.
    pub x: u32,
    /// Top edge in source pixels.
    pub y: u32,
    /// Cropped width in source pixels.
    pub width: u32,
    /// Cropped height in source pixels.
    pub height: u32,
}

/// What the renderer must produce for the current geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderPlan {
    /// Raster size the renderer allocates.
    pub raster: Size,
    /// Source region to sample, `None` to use the whole source.
    pub crop: Option<Crop>,
}

/// Plans the output raster for `source` inside `viewport`.
///
/// `max_height` applies the backend/quality budget (e.g. 720 px for Kitty,
/// 480 px for Sixel) and preserves the planned aspect ratio.
#[must_use]
pub fn plan_render(
    viewport: Viewport,
    source: Size,
    fit: Fit,
    max_height: Option<u32>,
) -> RenderPlan {
    let source = Size {
        width: source.width.max(1),
        height: source.height.max(1),
    };
    let area = viewport.pixel_size();

    match fit {
        Fit::Contain => RenderPlan {
            raster: cap_height(contain(area, source), max_height),
            crop: None,
        },
        Fit::Cover => {
            let raster = cap_height(area, max_height);
            RenderPlan {
                raster,
                crop: cover_crop(raster, source),
            }
        }
        Fit::Stretch => RenderPlan {
            raster: cap_height(area, max_height),
            crop: None,
        },
    }
}

/// Largest size that fits inside `area` while keeping `source`'s aspect ratio.
fn contain(area: Size, source: Size) -> Size {
    let width_limited = u64::from(area.width) * u64::from(source.height)
        <= u64::from(area.height) * u64::from(source.width);
    if width_limited {
        Size {
            width: area.width,
            height: scale(source.height, area.width, source.width),
        }
    } else {
        Size {
            width: scale(source.width, area.height, source.height),
            height: area.height,
        }
    }
}

/// Centered source rectangle whose aspect ratio matches the raster.
fn cover_crop(raster: Size, source: Size) -> Option<Crop> {
    let raster_wider = u64::from(raster.width) * u64::from(source.height)
        >= u64::from(raster.height) * u64::from(source.width);
    if raster_wider {
        let height = scale(source.width, raster.height, raster.width);
        if height >= source.height {
            return None;
        }
        Some(Crop {
            x: 0,
            y: (source.height - height) / 2,
            width: source.width,
            height,
        })
    } else {
        let width = scale(source.height, raster.width, raster.height);
        if width >= source.width {
            return None;
        }
        Some(Crop {
            x: (source.width - width) / 2,
            y: 0,
            width,
            height: source.height,
        })
    }
}

fn cap_height(size: Size, max_height: Option<u32>) -> Size {
    match max_height {
        Some(cap) if cap > 0 && size.height > cap => Size {
            width: scale(size.width, cap, size.height),
            height: cap,
        },
        _ => size,
    }
}

/// `value * numerator / denominator`, rounded, never zero.
fn scale(value: u32, numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        return 1;
    }
    let scaled = (u64::from(value) * u64::from(numerator) + u64::from(denominator) / 2)
        / u64::from(denominator);
    u32::try_from(scaled.max(1)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{CellSize, Crop};
    use super::{DEFAULT_CELL, Fit, Size, Viewport, plan_render};

    fn viewport(cols: u16, rows: u16) -> Viewport {
        Viewport::new(cols, rows, DEFAULT_CELL).unwrap()
    }

    #[test]
    fn viewport_rejects_zero_dimensions() {
        assert!(Viewport::new(0, 24, DEFAULT_CELL).is_none());
        assert!(Viewport::new(80, 0, DEFAULT_CELL).is_none());
        assert!(CellSize::new(0, 16).is_none());
    }

    #[test]
    fn pixel_size_multiplies_cells_by_cell_size() {
        assert_eq!(viewport(80, 24).pixel_size(), Size::new(640, 384));
    }

    #[test]
    fn contain_letterboxes_16_9_into_4_3() {
        let plan = plan_render(viewport(80, 24), Size::new(1920, 1080), Fit::Contain, None);
        assert_eq!(plan.raster, Size::new(640, 360));
        assert_eq!(plan.crop, None);
    }

    #[test]
    fn contain_respects_height_cap() {
        let plan = plan_render(
            viewport(80, 24),
            Size::new(1920, 1080),
            Fit::Contain,
            Some(240),
        );
        assert_eq!(plan.raster, Size::new(427, 240));
    }

    #[test]
    fn contain_handles_portrait_sources() {
        let plan = plan_render(viewport(80, 24), Size::new(1080, 1920), Fit::Contain, None);
        assert_eq!(plan.raster, Size::new(216, 384));
    }

    #[test]
    fn cover_fills_viewport_and_crops_the_sides() {
        let plan = plan_render(viewport(80, 24), Size::new(1920, 1080), Fit::Cover, None);
        assert_eq!(plan.raster, Size::new(640, 384));
        assert_eq!(
            plan.crop,
            Some(Crop {
                x: 60,
                y: 0,
                width: 1800,
                height: 1080
            })
        );
    }

    #[test]
    fn cover_needs_no_crop_when_aspect_matches() {
        let plan = plan_render(viewport(80, 24), Size::new(1280, 768), Fit::Cover, None);
        assert_eq!(plan.raster, Size::new(640, 384));
        assert_eq!(plan.crop, None);
    }

    #[test]
    fn cover_keeps_aspect_when_capped() {
        let plan = plan_render(
            viewport(80, 24),
            Size::new(1920, 1080),
            Fit::Cover,
            Some(240),
        );
        assert_eq!(plan.raster, Size::new(400, 240));
        assert_eq!(
            plan.crop,
            Some(Crop {
                x: 60,
                y: 0,
                width: 1800,
                height: 1080
            })
        );
    }

    #[test]
    fn stretch_uses_the_whole_viewport() {
        let plan = plan_render(viewport(80, 24), Size::new(1920, 1080), Fit::Stretch, None);
        assert_eq!(plan.raster, Size::new(640, 384));
    }

    #[test]
    fn degenerate_source_does_not_panic_or_divide_by_zero() {
        let plan = plan_render(viewport(1, 1), Size::new(0, 0), Fit::Contain, Some(0));
        // A 1:1 source fits inside the 8x16 cell as 8x8; a zero height cap is ignored.
        assert_eq!(plan.raster, Size::new(8, 8));
        assert_eq!(plan.crop, None);
    }
}
