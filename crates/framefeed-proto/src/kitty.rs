//! Kitty graphics protocol encoder.
//!
//! Wire format: `ESC _ G <k=v,...> ; <base64 payload> ESC \` (an APC string).
//!
//! This encoder transmits and displays a frame in one action (`a=T`), reusing a
//! caller-provided image id so the terminal replaces the image in place instead
//! of accumulating new ones. `C=1` prevents cursor movement, and `q=2` silences
//! terminal responses (we never wait for an ack in the playback loop).
//!
//! Not implemented yet: shared-memory / temp-file transports (`t=s`, `t=t`) and
//! the animation/delta-frame actions (`a=f`, `a=a`, `a=c`). Both are M6 items
//! tracked in `docs/ROADMAP.md`; see `docs/PROTOCOLS.md` for the rationale.

use std::error::Error;
use std::fmt;
use std::io::Write;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression as ZlibLevel;
use flate2::write::ZlibEncoder as ZlibWriter;
use framefeed_core::Raster;

/// Largest base64 chunk the protocol allows in one escape code.
const MAX_CHUNK: usize = 4096;

/// Payload compression applied before base64 encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// Send raw RGB bytes (useful for golden tests and tiny frames).
    None,
    /// RFC 1950 zlib (`o=z`), the default for real frames.
    Zlib,
}

/// Encoder configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KittyOptions {
    /// Image id to transmit/place. Must be non-zero; reuse it across frames.
    pub image_id: u32,
    /// Payload compression.
    pub compression: Compression,
    /// Suppress terminal responses (`q=2`).
    pub quiet: bool,
}

impl Default for KittyOptions {
    fn default() -> Self {
        Self {
            image_id: 1,
            compression: Compression::Zlib,
            quiet: true,
        }
    }
}

/// Encoding failure.
#[derive(Debug)]
pub enum EncodeError {
    /// The raster has no pixels, which cannot be transmitted.
    EmptyRaster,
    /// Zero is not a valid Kitty image id.
    InvalidImageId,
    /// The terminal area was zero cells in a dimension.
    InvalidArea,
    /// Writing into an in-memory buffer failed (should not happen).
    Io(std::io::Error),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRaster => f.write_str("cannot encode an empty raster"),
            Self::InvalidImageId => f.write_str("kitty image id must be non-zero"),
            Self::InvalidArea => f.write_str("image placement needs a non-zero area"),
            Self::Io(err) => write!(f, "failed to build escape sequence: {err}"),
        }
    }
}

impl Error for EncodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EncodeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Kitty graphics encoder with reusable scratch buffers.
///
/// One instance per terminal image id; all buffers are reused between frames so
/// the steady state performs no allocations.
#[derive(Debug)]
pub struct KittyEncoder {
    options: KittyOptions,
    compressed: Vec<u8>,
    encoded: Vec<u8>,
}

impl KittyEncoder {
    /// Creates an encoder for the given options.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError::InvalidImageId`] when `image_id` is zero.
    pub fn new(options: KittyOptions) -> Result<Self, EncodeError> {
        if options.image_id == 0 {
            return Err(EncodeError::InvalidImageId);
        }
        Ok(Self {
            options,
            compressed: Vec::new(),
            encoded: Vec::new(),
        })
    }

    /// Options this encoder was built with.
    #[must_use]
    pub const fn options(&self) -> KittyOptions {
        self.options
    }

    /// Encodes one frame as `a=T` with placement over `cols` x `rows` cells.
    ///
    /// Appends the complete escape sequence(s) to `out`: transmission is chunked
    /// at the protocol's 4096-byte base64 limit.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty raster, a zero-sized placement area, or an
    /// impossible in-memory write.
    pub fn encode_frame(
        &mut self,
        raster: &Raster,
        cols: u16,
        rows: u16,
        out: &mut Vec<u8>,
    ) -> Result<(), EncodeError> {
        if raster.width() == 0 || raster.height() == 0 {
            return Err(EncodeError::EmptyRaster);
        }
        if cols == 0 || rows == 0 {
            return Err(EncodeError::InvalidArea);
        }

        let payload = match self.options.compression {
            Compression::None => raster.pixels(),
            Compression::Zlib => {
                self.compressed.clear();
                let mut writer = ZlibWriter::new(&mut self.compressed, ZlibLevel::fast());
                writer.write_all(raster.pixels())?;
                writer.finish()?;
                &self.compressed
            }
        };

        let encoded_len = payload.len().div_ceil(3).saturating_mul(4);
        self.encoded.clear();
        self.encoded.resize(encoded_len, 0);
        let written = STANDARD
            .encode_slice(payload, &mut self.encoded)
            .map_err(|err| EncodeError::Io(std::io::Error::other(err.to_string())))?;
        self.encoded.truncate(written);
        let mut rest = self.encoded.as_slice();
        let mut first = true;
        loop {
            let take = rest.len().min(MAX_CHUNK);
            let (chunk, tail) = rest.split_at(take);
            let more = !tail.is_empty();

            // `m` belongs to the escape carrying this chunk: it tells the terminal
            // that more payload follows. Only the first chunk repeats the full
            // parameter set; later ones must carry just `m` (and optionally `q`).
            if first {
                out.extend_from_slice(b"\x1b_Ga=T,f=24");
                write!(
                    out,
                    ",s={},v={},i={},c={},r={},C=1",
                    raster.width(),
                    raster.height(),
                    self.options.image_id,
                    cols,
                    rows
                )?;
                if self.options.quiet {
                    out.extend_from_slice(b",q=2");
                }
                if self.options.compression == Compression::Zlib {
                    out.extend_from_slice(b",o=z");
                }
                write!(out, ",m={};", u8::from(more))?;
            } else {
                write!(out, "\x1b_Gm={};", u8::from(more))?;
            }

            out.extend_from_slice(chunk);
            out.extend_from_slice(b"\x1b\\");
            if !more {
                break;
            }
            rest = tail;
            first = false;
        }
        Ok(())
    }

    /// Deletes this image and frees the terminal-side pixel data.
    pub fn delete(&self, out: &mut Vec<u8>) {
        // `d=I` frees the data for the given id; `q=2` silences the response.
        let _ = write!(out, "\x1b_Ga=d,d=I,i={},q=2\x1b\\", self.options.image_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{Compression, EncodeError, KittyEncoder, KittyOptions, MAX_CHUNK};
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use flate2::read::ZlibDecoder;
    use framefeed_core::Raster;
    use std::io::Read;

    fn raster(width: u32, height: u32) -> Raster {
        let mut raster = Raster::new(width, height);
        for (index, byte) in raster.pixels_mut().iter_mut().enumerate() {
            *byte = u8::try_from(index % 251).unwrap();
        }
        raster
    }

    /// Splits an APC stream into `(control data, payload)` pairs.
    fn escapes(bytes: &[u8]) -> Vec<(String, String)> {
        let text = std::str::from_utf8(bytes).expect("escape sequences are ASCII");
        let mut result = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("\x1b_G") {
            let after = &rest[start + 3..];
            let end = after.find("\x1b\\").expect("unterminated APC");
            let body = &after[..end];
            let (control, payload) = body.split_once(';').expect("missing ';'");
            result.push((control.to_string(), payload.to_string()));
            rest = &after[end + 2..];
        }
        result
    }

    #[test]
    fn rejects_zero_image_id() {
        let err = KittyEncoder::new(KittyOptions {
            image_id: 0,
            ..KittyOptions::default()
        })
        .unwrap_err();
        assert!(matches!(err, EncodeError::InvalidImageId));
    }

    #[test]
    fn encodes_exact_bytes_without_compression() {
        let pixels = vec![
            255, 0, 0, // red
            0, 255, 0, // green
            0, 0, 255, // blue
            255, 255, 255, // white
        ];
        let raster = Raster::from_rgb(2, 2, pixels).unwrap();
        let mut encoder = KittyEncoder::new(KittyOptions {
            image_id: 42,
            compression: Compression::None,
            quiet: true,
        })
        .unwrap();

        let mut out = Vec::new();
        encoder.encode_frame(&raster, 1, 1, &mut out).unwrap();

        let expected_payload = STANDARD.encode([255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        let expected =
            format!("\x1b_Ga=T,f=24,s=2,v=2,i=42,c=1,r=1,C=1,q=2,m=0;{expected_payload}\x1b\\");
        assert_eq!(String::from_utf8(out).unwrap(), expected);
    }

    #[test]
    fn chunks_payload_at_protocol_limit() {
        let raster = raster(64, 64); // 12288 RGB bytes -> 16384 base64 chars
        let mut encoder = KittyEncoder::new(KittyOptions {
            image_id: 7,
            compression: Compression::None,
            quiet: true,
        })
        .unwrap();
        let mut out = Vec::new();
        encoder.encode_frame(&raster, 4, 2, &mut out).unwrap();

        let parts = escapes(&out);
        assert_eq!(parts.len(), 4, "expected 4 chunks");
        assert!(parts[0].0.contains("a=T"));
        assert!(parts[0].0.contains("c=4"));
        assert!(parts[0].0.contains("r=2"));
        assert!(parts[0].0.contains("C=1"));
        for (index, (control, payload)) in parts.iter().enumerate() {
            let last = index + 1 == parts.len();
            assert!(
                !control.contains("a=T") || index == 0,
                "only the first chunk carries the action"
            );
            assert!(payload.len() <= MAX_CHUNK);
            assert_eq!(control.contains("m=1"), !last);
            if !last {
                assert_eq!(payload.len() % 4, 0);
            }
        }

        let joined: String = parts.iter().map(|(_, payload)| payload.as_str()).collect();
        let decoded = STANDARD.decode(joined).unwrap();
        assert_eq!(decoded, raster.pixels());
    }

    #[test]
    fn compresses_with_zlib_and_still_roundtrips() {
        let raster = raster(32, 32);
        let mut encoder = KittyEncoder::new(KittyOptions::default()).unwrap();
        let mut out = Vec::new();
        encoder.encode_frame(&raster, 4, 2, &mut out).unwrap();

        let parts = escapes(&out);
        assert!(parts[0].0.contains("o=z"));

        let joined: String = parts.iter().map(|(_, payload)| payload.as_str()).collect();
        let compressed = STANDARD.decode(joined).unwrap();
        let mut inflated = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut inflated)
            .unwrap();
        assert_eq!(inflated, raster.pixels());
    }

    #[test]
    fn delete_frees_the_image() {
        let encoder = KittyEncoder::new(KittyOptions {
            image_id: 9,
            ..KittyOptions::default()
        })
        .unwrap();
        let mut out = Vec::new();
        encoder.delete(&mut out);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b_Ga=d,d=I,i=9,q=2\x1b\\"
        );
    }

    #[test]
    fn rejects_degenerate_input() {
        let mut encoder = KittyEncoder::new(KittyOptions::default()).unwrap();
        let mut out = Vec::new();
        let empty = Raster::from_rgb(0, 1, Vec::new()).unwrap_err();
        assert_eq!(empty.expected, 0);
        assert!(matches!(
            encoder.encode_frame(&Raster::new(1, 0), 1, 1, &mut out),
            Err(EncodeError::EmptyRaster)
        ));
        assert!(matches!(
            encoder.encode_frame(&Raster::new(1, 1), 0, 1, &mut out),
            Err(EncodeError::InvalidArea)
        ));
    }
}
