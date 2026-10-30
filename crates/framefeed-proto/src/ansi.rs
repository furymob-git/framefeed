//! ANSI truecolor encoder using the upper half block (`▀`).
//!
//! Two vertical pixels per character cell: foreground paints the top pixel,
//! background the bottom one. This works on any 24-bit color terminal, which
//! makes it the universal fallback and the sane default for `framefeed serve`
//! (`curl` output). SGR sequences are only emitted when a color changes, and a
//! reset ends each row so a broken write cannot bleed into the next line.

use std::io::Write as _;

use framefeed_core::Raster;

const RESET: &[u8] = b"\x1b[0m";
const HALF_BLOCK: &[u8] = "▀".as_bytes();

/// Encoder state; reuse one instance across frames.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnsiEncoder {
    foreground: Option<(u8, u8, u8)>,
    background: Option<(u8, u8, u8)>,
}

impl AnsiEncoder {
    /// Creates an encoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one frame: cursor home, then `height / 2` text rows.
    ///
    /// The output assumes the cursor is at the top-left of the drawing area (the
    /// alternate screen, or a `CSI H` issued by the caller).
    pub fn encode_frame(&mut self, raster: &Raster, out: &mut Vec<u8>) {
        let width = usize::try_from(raster.width()).unwrap_or_default();
        let height = usize::try_from(raster.height()).unwrap_or_default();
        if width == 0 || height == 0 {
            return;
        }
        let pixels = raster.pixels();

        out.extend_from_slice(b"\x1b[H");
        self.foreground = None;
        self.background = None;

        let mut y = 0;
        while y < height {
            for x in 0..width {
                let top = pixel(pixels, width, x, y);
                let bottom = if y + 1 < height {
                    pixel(pixels, width, x, y + 1)
                } else {
                    (0, 0, 0)
                };
                if self.foreground != Some(top) {
                    push_sgr(out, 38, top);
                    self.foreground = Some(top);
                }
                if self.background != Some(bottom) {
                    push_sgr(out, 48, bottom);
                    self.background = Some(bottom);
                }
                out.extend_from_slice(HALF_BLOCK);
            }
            out.extend_from_slice(RESET);
            out.extend_from_slice(b"\r\n");
            self.foreground = None;
            self.background = None;
            y += 2;
        }
    }
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> (u8, u8, u8) {
    let offset = (y * width + x) * 3;
    (pixels[offset], pixels[offset + 1], pixels[offset + 2])
}

fn push_sgr(out: &mut Vec<u8>, layer: u8, rgb: (u8, u8, u8)) {
    // Writing into a Vec cannot fail; the buffer only grows.
    write!(out, "\x1b[{layer};2;{};{};{}m", rgb.0, rgb.1, rgb.2)
        .expect("writing to a Vec cannot fail");
}

#[cfg(test)]
mod tests {
    use super::AnsiEncoder;
    use framefeed_core::Raster;

    #[test]
    fn encodes_red_green_over_blue_white_exactly() {
        let pixels = vec![
            255, 0, 0, // top-left red
            0, 255, 0, // top-right green
            0, 0, 255, // bottom-left blue
            255, 255, 255, // bottom-right white
        ];
        let raster = Raster::from_rgb(2, 2, pixels).unwrap();
        let mut out = Vec::new();
        AnsiEncoder::new().encode_frame(&raster, &mut out);

        let expected = concat!(
            "\x1b[H",
            "\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀",
            "\x1b[38;2;0;255;0m\x1b[48;2;255;255;255m▀",
            "\x1b[0m\r\n",
        );
        assert_eq!(String::from_utf8(out).unwrap(), expected);
    }

    #[test]
    fn reuses_sgr_when_colors_repeat() {
        let raster = Raster::from_rgb(2, 1, vec![9, 9, 9, 9, 9, 9]).unwrap();
        let mut out = Vec::new();
        AnsiEncoder::new().encode_frame(&raster, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text.matches("\x1b[38;2;9;9;9m").count(),
            1,
            "one fg sequence for a uniform row: {text:?}"
        );
        assert_eq!(text.matches('▀').count(), 2);
    }

    #[test]
    fn odd_height_pads_the_last_row_with_black() {
        let raster = Raster::from_rgb(1, 3, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
        let mut out = Vec::new();
        AnsiEncoder::new().encode_frame(&raster, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text.matches("\r\n").count(),
            2,
            "3 pixel rows produce 2 text rows"
        );
        assert!(
            text.contains("\x1b[48;2;0;0;0m"),
            "last row has no bottom pixel"
        );
    }

    #[test]
    fn empty_raster_is_a_no_op() {
        let raster = Raster::new(0, 0);
        let mut out = Vec::new();
        AnsiEncoder::new().encode_frame(&raster, &mut out);
        assert!(out.is_empty());
    }
}
