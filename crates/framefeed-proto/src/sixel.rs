//! Sixel encoder (DEC graphics).
//!
//! Sixel is real pixel graphics: the terminal rasterises an indexed-colour bitmap
//! sent as `ESC P q … ESC \`. It is the backend that works on Windows Terminal
//! 1.22+, foot, WezTerm, Konsole, mintty and iTerm2 — none of which speak Kitty
//! graphics.
//!
//! Format notes: each band is six pixel rows; a band is painted one colour at a
//! time (`#n` selects a palette entry, characters encode six vertical pixels,
//! `$` returns to the left margin, `-` starts the next band). Repeated characters
//! are compressed with `!count`.
//!
//! Palette: fixed 240 entries = 6×6×6 colour cube + a 24-step grey ramp. That is
//! cheap to quantise (no nearest-neighbour search) and good enough for video;
//! adaptive palettes and dithering are tracked for M6.

use std::io::Write as _;

use framefeed_core::Raster;

/// Steps per channel in the colour cube.
const CUBE_STEPS: u16 = 6;
/// Cube entries (before the grey ramp).
const CUBE_ENTRIES: usize = 216;
/// Grey ramp entries.
const GREY_ENTRIES: usize = 24;
/// Total palette entries.
pub const PALETTE_SIZE: usize = CUBE_ENTRIES + GREY_ENTRIES;

/// Sixel encoder with reusable per-band scratch buffers.
#[derive(Debug)]
pub struct SixelEncoder {
    band: Vec<u8>,
    present: [bool; PALETTE_SIZE],
    colors: Vec<u8>,
    width: usize,
}

impl Default for SixelEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SixelEncoder {
    /// Creates an encoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            band: Vec::new(),
            present: [false; PALETTE_SIZE],
            colors: Vec::new(),
            width: 0,
        }
    }

    /// Appends one full-frame sixel image.
    ///
    /// Sixel has no in-place update, so every frame repaints the whole raster;
    /// the backend budget keeps the raster small because of that.
    pub fn encode_frame(&mut self, raster: &Raster, out: &mut Vec<u8>) {
        let width = usize::try_from(raster.width()).unwrap_or_default();
        let height = usize::try_from(raster.height()).unwrap_or_default();
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;

        out.extend_from_slice(b"\x1bPq");
        // Raster attributes: pixel aspect 1:1, then width and height.
        let _ = write!(out, "\"1;1;{width};{height}");
        write_palette(out);

        let pixels = raster.pixels();
        let mut y = 0;
        while y < height {
            let rows = (height - y).min(6);
            self.band.clear();
            self.band.reserve(width * rows);
            self.present = [false; PALETTE_SIZE];
            for row in 0..rows {
                let start = (y + row) * width * 3;
                for pixel in pixels[start..start + width * 3].chunks_exact(3) {
                    let index = quantize(pixel[0], pixel[1], pixel[2]);
                    self.band.push(index);
                    self.present[usize::from(index)] = true;
                }
            }

            self.colors.clear();
            self.colors.extend(
                (0..PALETTE_SIZE)
                    .filter(|index| self.present[*index])
                    .map(|index| u8::try_from(index).expect("palette size fits in u8")),
            );
            let color_count = self.colors.len();

            for (position, color) in self.colors.iter().enumerate() {
                let _ = write!(out, "#{color}");
                let mut x = 0;
                while x < width {
                    let bits = self.column_bits(x, rows, *color);
                    let mut run = 1;
                    while x + run < width && self.column_bits(x + run, rows, *color) == bits {
                        run += 1;
                    }
                    if run > 3 {
                        let _ = write!(out, "!{run}");
                        out.push(0x3F + bits);
                    } else {
                        for _ in 0..run {
                            out.push(0x3F + bits);
                        }
                    }
                    x += run;
                }
                out.push(if position + 1 < color_count {
                    b'$'
                } else {
                    b'-'
                });
            }

            y += 6;
        }

        out.extend_from_slice(b"\x1b\\");
    }

    fn column_bits(&self, x: usize, rows: usize, color: u8) -> u8 {
        let mut bits = 0u8;
        for row in 0..rows {
            if self.band[row * self.width + x] == color {
                bits |= 1 << row;
            }
        }
        bits
    }
}

fn write_palette(out: &mut Vec<u8>) {
    for index in 0..PALETTE_SIZE {
        let entry = u8::try_from(index).expect("palette size fits in u8");
        let (r, g, b) = palette_color(entry);
        let _ = write!(
            out,
            "#{index};2;{};{};{}",
            percent(r),
            percent(g),
            percent(b)
        );
    }
}

/// Maps a pixel to a palette entry: cube for saturated colours, grey ramp for
/// near-neutral ones (which is where banding is most visible).
fn quantize(r: u8, g: u8, b: u8) -> u8 {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max - min < 12 {
        let luma = (u16::from(r) * 77 + u16::from(g) * 150 + u16::from(b) * 29) >> 8;
        let step = (u32::from(luma) * (GREY_ENTRIES as u32 - 1) + 127) / 255;
        let step = u8::try_from(step.min(GREY_ENTRIES as u32 - 1)).expect("step fits in u8");
        return u8::try_from(CUBE_ENTRIES).expect("cube size fits in u8") + step;
    }

    let ri = (u16::from(r) * (CUBE_STEPS - 1) + 127) / 255;
    let gi = (u16::from(g) * (CUBE_STEPS - 1) + 127) / 255;
    let bi = (u16::from(b) * (CUBE_STEPS - 1) + 127) / 255;
    let index = ri * 36 + gi * 6 + bi;
    u8::try_from(index).expect("cube index fits in u8")
}

fn palette_color(index: u8) -> (u8, u8, u8) {
    let index = usize::from(index);
    if index < CUBE_ENTRIES {
        let r = u16::try_from(index / 36).expect("cube row fits") * 51;
        let g = u16::try_from((index % 36) / 6).expect("cube column fits") * 51;
        let b = u16::try_from(index % 6).expect("cube channel fits") * 51;
        (
            u8::try_from(r).unwrap_or(255),
            u8::try_from(g).unwrap_or(255),
            u8::try_from(b).unwrap_or(255),
        )
    } else {
        let step = u16::try_from(index - CUBE_ENTRIES).unwrap_or(0);
        let level = step * 255 / (GREY_ENTRIES as u16 - 1);
        let level = u8::try_from(level).unwrap_or(255);
        (level, level, level)
    }
}

/// Sixel colour components are percentages.
fn percent(value: u8) -> u16 {
    (u16::from(value) * 100 + 127) / 255
}

#[cfg(test)]
mod tests {
    use super::{PALETTE_SIZE, SixelEncoder, palette_color, quantize};
    use framefeed_core::Raster;

    /// Builds a raster from palette entries, returning the expected indices after
    /// quantisation (near-neutral colours land on the grey ramp).
    fn raster_from_indices(
        width: usize,
        height: usize,
        pattern: impl Fn(usize, usize) -> u8,
    ) -> (Raster, Vec<u8>) {
        let mut pixels = Vec::with_capacity(width * height * 3);
        let mut expected = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let (r, g, b) = palette_color(pattern(x, y));
                pixels.extend_from_slice(&[r, g, b]);
                expected.push(quantize(r, g, b));
            }
        }
        let raster = Raster::from_rgb(
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
            pixels,
        )
        .unwrap();
        (raster, expected)
    }

    fn number(text: &str, start: usize) -> (usize, usize) {
        let bytes = text.as_bytes();
        let mut index = start;
        let mut value = 0usize;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            value = value * 10 + usize::from(bytes[index] - b'0');
            index += 1;
        }
        (value, index)
    }

    fn paint(
        character: u8,
        width: usize,
        height: usize,
        x: usize,
        y: usize,
        color: usize,
        grid: &mut [u8],
    ) {
        let bits = character.wrapping_sub(0x3F);
        for row in 0..6 {
            if bits & (1 << row) != 0 && y + row < height && x < width {
                grid[(y + row) * width + x] = u8::try_from(color).unwrap_or(u8::MAX);
            }
        }
    }

    /// Minimal decoder for the subset this encoder emits.
    fn decode(bytes: &[u8], width: usize, height: usize) -> Vec<u8> {
        let text = std::str::from_utf8(bytes).expect("sixel output is ASCII");
        let body = text.strip_prefix("\x1bPq").expect("DCS header");
        let body = body.strip_suffix("\x1b\\").expect("ST terminator");
        let raw = body.as_bytes();
        let mut index = 0;
        // Skip raster attributes before the first palette entry.
        if body.starts_with('"') {
            while index < raw.len() && raw[index] != b'#' {
                index += 1;
            }
        }

        let mut grid = vec![u8::MAX; width * height];
        let mut color = 0usize;
        let mut x = 0usize;
        let mut y = 0usize;
        while index < raw.len() {
            match raw[index] {
                b'#' => {
                    let (value, mut next) = number(body, index + 1);
                    color = value;
                    // Skip `;2;r;g;b` parameters.
                    while next < raw.len() && (raw[next] == b';' || raw[next].is_ascii_digit()) {
                        next += 1;
                    }
                    index = next;
                }
                b'!' => {
                    let (count, next) = number(body, index + 1);
                    index = next;
                    let character = raw[index];
                    index += 1;
                    for _ in 0..count {
                        paint(character, width, height, x, y, color, &mut grid);
                        x += 1;
                    }
                }
                b'$' => {
                    x = 0;
                    index += 1;
                }
                b'-' => {
                    y += 6;
                    x = 0;
                    index += 1;
                }
                character => {
                    paint(character, width, height, x, y, color, &mut grid);
                    x += 1;
                    index += 1;
                }
            }
        }
        grid
    }

    #[test]
    fn writes_header_terminator_and_one_band_per_six_rows() {
        let (raster, _) = raster_from_indices(10, 7, |x, y| u8::try_from((x + y) % 5).unwrap());
        let mut out = Vec::new();
        SixelEncoder::new().encode_frame(&raster, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("\x1bPq\"1;1;10;7"), "{text:?}");
        assert!(text.ends_with("\x1b\\"));
        assert_eq!(text.matches('-').count(), 2, "7 rows span two bands");
        for index in [0, PALETTE_SIZE - 1] {
            assert!(
                text.contains(&format!("#{index};2;")),
                "palette entry {index} missing"
            );
        }
    }

    #[test]
    fn roundtrips_quantised_pixels() {
        let (raster, expected) = raster_from_indices(10, 7, |x, y| match (x / 3 + y) % 3 {
            0 => 0,   // black -> grey ramp
            1 => 7,   // dark teal in the cube
            _ => 100, // mid cube colour
        });
        let mut out = Vec::new();
        SixelEncoder::new().encode_frame(&raster, &mut out);
        assert_eq!(decode(&out, 10, 7), expected);
    }

    #[test]
    fn run_length_encodes_uniform_bands() {
        let (raster, _) = raster_from_indices(64, 6, |_, _| 100);
        let mut out = Vec::new();
        SixelEncoder::new().encode_frame(&raster, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("!64"),
            "uniform band should use RLE: {text:?}"
        );
    }

    #[test]
    fn palette_endpoints_are_consistent() {
        assert_eq!(palette_color(0), (0, 0, 0));
        assert_eq!(palette_color(215), (255, 255, 255));
        assert_eq!(palette_color(239), (255, 255, 255));
        assert_eq!(quantize(255, 255, 255), 239, "white uses the grey ramp");
        assert_eq!(quantize(0, 0, 0), 216, "black uses the grey ramp");
    }
}
