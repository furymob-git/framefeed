//! Pixel containers: [`Frame`] comes out of a decoder, [`Raster`] goes into an
//! encoder.

use std::error::Error;
use std::fmt;
use std::time::Duration;

/// Pixel layout of a decoded [`Frame`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8 bits per channel, alpha included (the decoder sidecar's native output).
    Rgba8,
    /// 8 bits per channel, no alpha (what the Kitty/Sixel encoders transmit).
    Rgb8,
}

impl PixelFormat {
    /// Bytes used by one pixel in this format.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8 => 4,
            Self::Rgb8 => 3,
        }
    }

    /// Human-readable name, used by diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rgba8 => "rgba8",
            Self::Rgb8 => "rgb8",
        }
    }
}

/// Buffer length did not match the declared dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidBuffer {
    /// Number of bytes the dimensions require.
    pub expected: usize,
    /// Number of bytes actually provided.
    pub actual: usize,
}

impl fmt::Display for InvalidBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "buffer length mismatch: expected {} bytes, got {}",
            self.expected, self.actual
        )
    }
}

impl Error for InvalidBuffer {}

/// A decoded video frame: tightly packed pixels plus presentation timestamp.
#[derive(Clone, Debug)]
pub struct Frame {
    width: u32,
    height: u32,
    stride: u32,
    format: PixelFormat,
    data: Vec<u8>,
    pts: Option<Duration>,
}

impl Frame {
    /// Builds a tightly packed frame.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidBuffer`] when `data` is not exactly
    /// `width * height * bytes_per_pixel` bytes, or when a dimension is zero.
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        data: Vec<u8>,
        pts: Option<Duration>,
    ) -> Result<Self, InvalidBuffer> {
        let row = usize::try_from(width)
            .unwrap_or_default()
            .saturating_mul(format.bytes_per_pixel());
        let expected = row.saturating_mul(usize::try_from(height).unwrap_or_default());
        if width == 0 || height == 0 || data.len() != expected {
            return Err(InvalidBuffer {
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            stride: u32::try_from(row).unwrap_or(u32::MAX),
            format,
            data,
            pts,
        })
    }

    /// Frame width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Frame height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per row.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Pixel format of the buffer.
    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    /// Tightly packed pixel bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Presentation timestamp, when the source provides one.
    #[must_use]
    pub const fn pts(&self) -> Option<Duration> {
        self.pts
    }
}

/// An output-sized RGB24 image, ready for a display encoder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Raster {
    /// Allocates a black raster of the given size.
    ///
    /// Dimensions are expected to come from [`crate::plan_render`], i.e. bounded
    /// by the terminal size and the backend budget.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let pixels = Self::buffer_len(width, height);
        Self {
            width,
            height,
            pixels: vec![0; pixels],
        }
    }

    /// Wraps an existing RGB24 buffer.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidBuffer`] when the buffer or dimensions are inconsistent.
    pub fn from_rgb(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, InvalidBuffer> {
        let expected = Self::buffer_len(width, height);
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err(InvalidBuffer {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Raster width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Raster height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Width and height as a [`crate::Size`].
    #[must_use]
    pub const fn size(&self) -> crate::Size {
        crate::Size {
            width: self.width,
            height: self.height,
        }
    }

    /// RGB24 pixel bytes, three bytes per pixel, row-major.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Mutable access for the renderer that fills this raster.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Copies tightly packed RGBA pixels into this raster, dropping alpha.
    ///
    /// Decoders hand us RGBA; the encoders transmit RGB24.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidBuffer`] when `rgba` is not `width * height * 4` bytes.
    pub fn write_rgba(&mut self, rgba: &[u8]) -> Result<(), InvalidBuffer> {
        let expected = self.pixels.len() / 3 * 4;
        if rgba.len() != expected {
            return Err(InvalidBuffer {
                expected,
                actual: rgba.len(),
            });
        }
        for (destination, source) in self.pixels.chunks_exact_mut(3).zip(rgba.chunks_exact(4)) {
            destination.copy_from_slice(&source[..3]);
        }
        Ok(())
    }

    fn buffer_len(width: u32, height: u32) -> usize {
        usize::try_from(width)
            .unwrap_or_default()
            .saturating_mul(usize::try_from(height).unwrap_or_default())
            .saturating_mul(3)
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, InvalidBuffer, PixelFormat, Raster};

    #[test]
    fn frame_rejects_wrong_buffer_length() {
        let err = Frame::new(2, 2, PixelFormat::Rgba8, vec![0; 15], None).unwrap_err();
        assert_eq!(
            err,
            InvalidBuffer {
                expected: 16,
                actual: 15
            }
        );
    }

    #[test]
    fn frame_rejects_zero_dimensions() {
        assert!(Frame::new(0, 4, PixelFormat::Rgb8, Vec::new(), None).is_err());
    }

    #[test]
    fn frame_computes_stride_from_format() {
        let frame = Frame::new(3, 2, PixelFormat::Rgba8, vec![0; 24], None).unwrap();
        assert_eq!(frame.stride(), 12);
        assert_eq!(frame.width(), 3);
        assert_eq!(frame.pts(), None);
    }

    #[test]
    fn raster_exposes_rgb24_pixels() {
        let mut raster = Raster::new(2, 1);
        assert_eq!(raster.pixels().len(), 6);
        raster.pixels_mut()[0] = 255;
        assert_eq!(raster.pixels()[0], 255);
        assert_eq!(raster.size().width, 2);
    }

    #[test]
    fn raster_drops_alpha_from_rgba() {
        let mut raster = Raster::new(1, 1);
        raster.write_rgba(&[10, 20, 30, 255]).unwrap();
        assert_eq!(raster.pixels(), &[10u8, 20, 30][..]);
        assert!(raster.write_rgba(&[1, 2, 3]).is_err());
    }

    #[test]
    fn raster_validates_buffer_length() {
        assert!(Raster::from_rgb(2, 2, vec![0; 11]).is_err());
        assert!(Raster::from_rgb(2, 2, vec![0; 12]).is_ok());
    }
}
