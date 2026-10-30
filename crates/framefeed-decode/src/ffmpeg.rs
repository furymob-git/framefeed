//! ffmpeg sidecar decoder.

use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::process::{Child, ChildStdout, Command, Stdio};

use framefeed_core::Raster;

use crate::Decoder;

/// Metadata needed to pace and scale playback.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoMetadata {
    /// Coded width in pixels.
    pub width: u32,
    /// Coded height in pixels.
    pub height: u32,
    /// Average frame rate, guessed when the container does not declare one.
    pub fps: f64,
}

/// Requested output size for the decoder's scaler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSize {
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
    /// Fill the target and crop the overflow (otherwise letterbox/distort).
    pub crop: bool,
}

/// Decoder failure.
#[derive(Debug)]
pub enum DecodeError {
    /// Could not start `ffmpeg`/`ffprobe`.
    Spawn {
        /// Binary we tried to run.
        program: String,
        /// Underlying OS error.
        source: io::Error,
    },
    /// `ffprobe` exited non-zero or printed nothing usable.
    ProbeFailed(String),
    /// The container declares no usable video stream dimensions.
    InvalidMetadata(String),
    /// I/O failure while reading frames.
    Io(io::Error),
    /// The decoder stopped mid-frame.
    TruncatedFrame {
        /// Bytes read before EOF.
        got: usize,
        /// Bytes a full frame requires.
        expected: usize,
    },
    /// The internal raster could not accept the RGBA frame.
    Raster(framefeed_core::InvalidBuffer),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, source } => write!(
                f,
                "cannot run `{program}`: {source}. Install ffmpeg and make sure it is on PATH \
                 (or set FRAMEFEED_FFMPEG / FRAMEFEED_FFPROBE)"
            ),
            Self::ProbeFailed(message) => write!(f, "ffprobe failed: {message}"),
            Self::InvalidMetadata(message) => write!(f, "invalid video metadata: {message}"),
            Self::Io(error) => write!(f, "reading frames failed: {error}"),
            Self::TruncatedFrame { got, expected } => {
                write!(f, "decoder stopped mid-frame ({got} of {expected} bytes)")
            }
            Self::Raster(error) => write!(f, "frame does not fit the raster: {error}"),
        }
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Io(source) => Some(source),
            Self::Raster(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for DecodeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<framefeed_core::InvalidBuffer> for DecodeError {
    fn from(error: framefeed_core::InvalidBuffer) -> Self {
        Self::Raster(error)
    }
}

/// ffmpeg-backed decoder.
#[derive(Debug)]
pub struct FfmpegDecoder {
    child: Child,
    stdout: ChildStdout,
    metadata: VideoMetadata,
    rgba: Vec<u8>,
    raster: Raster,
}

impl FfmpegDecoder {
    /// Probes `source` and starts decoding.
    ///
    /// `output` asks swscale for an exact raster size (and optional cover-crop);
    /// `None` keeps the source resolution.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when ffprobe/ffmpeg are missing, the source has no
    /// usable video stream, or the processes cannot be started.
    pub fn open(source: &str, output: Option<OutputSize>) -> Result<Self, DecodeError> {
        let metadata = probe(source)?;
        let mut command = Command::new(ffmpeg_bin());
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostdin")
            .arg("-i")
            .arg(source)
            .args(["-an", "-sn", "-dn"]);

        let (width, height) = match output {
            Some(size) => {
                command.arg("-vf").arg(scale_filter(size));
                (size.width, size.height)
            }
            None => (metadata.width, metadata.height),
        };

        command
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = command.spawn().map_err(|source| DecodeError::Spawn {
            program: ffmpeg_bin(),
            source,
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DecodeError::Io(io::Error::other("ffmpeg stdout was not captured")))?;

        let frame_bytes = usize::try_from(width)
            .unwrap_or_default()
            .saturating_mul(usize::try_from(height).unwrap_or_default())
            .saturating_mul(4);
        if frame_bytes == 0 {
            return Err(DecodeError::InvalidMetadata(format!(
                "{width}x{height} is not a usable raster size"
            )));
        }

        Ok(Self {
            child,
            stdout,
            metadata,
            rgba: vec![0; frame_bytes],
            raster: Raster::new(width, height),
        })
    }
}

impl Decoder for FfmpegDecoder {
    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    fn next_frame(&mut self) -> Result<Option<&Raster>, DecodeError> {
        let filled = read_frame(&mut self.stdout, &mut self.rgba)?;
        if filled == 0 {
            return Ok(None);
        }
        if filled < self.rgba.len() {
            return Err(DecodeError::TruncatedFrame {
                got: filled,
                expected: self.rgba.len(),
            });
        }
        self.raster.write_rgba(&self.rgba)?;
        Ok(Some(&self.raster))
    }
}

impl Drop for FfmpegDecoder {
    fn drop(&mut self) {
        // Ctrl-C or an early exit must not leave a decoder chewing CPU.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ffmpeg_bin() -> String {
    std::env::var("FRAMEFEED_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string())
}

fn ffprobe_bin() -> String {
    std::env::var("FRAMEFEED_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string())
}

fn scale_filter(size: OutputSize) -> String {
    let (width, height) = (size.width, size.height);
    if size.crop {
        format!(
            "scale={width}:{height}:force_original_aspect_ratio=increase:flags=fast_bilinear,crop={width}:{height}"
        )
    } else {
        format!("scale={width}:{height}:flags=fast_bilinear")
    }
}

/// Probes `source` with ffprobe without starting a decode session.
///
/// Used by callers that need the source dimensions to plan a raster before
/// opening the decoder (the server sizes its stream per request).
///
/// # Errors
///
/// Returns [`DecodeError`] when ffprobe is missing, fails, or reports no video.
pub fn probe(source: &str) -> Result<VideoMetadata, DecodeError> {
    let output = Command::new(ffprobe_bin())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate,r_frame_rate",
            "-of",
            "default=noprint_wrappers=1",
            source,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|source| DecodeError::Spawn {
            program: ffprobe_bin(),
            source,
        })?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(DecodeError::ProbeFailed(if message.is_empty() {
            format!("ffprobe exited with {}", output.status)
        } else {
            message
        }));
    }

    parse_probe(&String::from_utf8_lossy(&output.stdout))
}

fn parse_probe(text: &str) -> Result<VideoMetadata, DecodeError> {
    let mut width = None;
    let mut height = None;
    let mut average_rate = None;
    let mut raw_rate = None;

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "width" => width = value.parse::<u32>().ok(),
            "height" => height = value.parse::<u32>().ok(),
            "avg_frame_rate" => average_rate = parse_rate(value),
            "r_frame_rate" => raw_rate = parse_rate(value),
            _ => {}
        }
    }

    let width = width.filter(|value| *value > 0);
    let height = height.filter(|value| *value > 0);
    let (Some(width), Some(height)) = (width, height) else {
        return Err(DecodeError::InvalidMetadata(text.trim().to_string()));
    };
    Ok(VideoMetadata {
        width,
        height,
        fps: average_rate.or(raw_rate).unwrap_or(30.0),
    })
}

/// Parses `30000/1001`, `25`, rejects `0/0`.
fn parse_rate(value: &str) -> Option<f64> {
    let (numerator, denominator) = match value.split_once('/') {
        Some((numerator, denominator)) => (
            numerator.trim().parse::<f64>().ok()?,
            denominator.trim().parse::<f64>().ok()?,
        ),
        None => (value.trim().parse::<f64>().ok()?, 1.0),
    };
    if denominator == 0.0 || numerator <= 0.0 {
        return None;
    }
    let fps = numerator / denominator;
    (fps.is_finite() && fps > 0.0).then_some(fps)
}

/// Reads one frame, returning 0 only for a clean end of stream.
fn read_frame(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::{OutputSize, VideoMetadata, parse_probe, parse_rate, read_frame, scale_filter};
    use std::io::Cursor;

    #[test]
    fn parses_ffprobe_key_value_output() {
        let metadata = parse_probe("width=1920\nheight=1080\navg_frame_rate=30000/1001\n").unwrap();
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert!((metadata.fps - 29.97).abs() < 0.01);
    }

    #[test]
    fn falls_back_to_raw_frame_rate_then_thirty() {
        let metadata =
            parse_probe("width=640\nheight=360\navg_frame_rate=0/0\nr_frame_rate=12\n").unwrap();
        assert!((metadata.fps - 12.0).abs() < f64::EPSILON);

        let metadata = parse_probe("width=640\nheight=360\n").unwrap();
        assert!((metadata.fps - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rejects_metadata_without_dimensions() {
        assert!(parse_probe("width=0\nheight=0\n").is_err());
        assert!(parse_probe("").is_err());
    }

    #[test]
    fn parses_rates_conservatively() {
        assert_eq!(parse_rate("25"), Some(25.0));
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("30/0"), None);
        assert_eq!(parse_rate("abc"), None);
        assert_eq!(parse_rate("-5"), None);
    }

    #[test]
    fn builds_scale_filters() {
        assert_eq!(
            scale_filter(OutputSize {
                width: 640,
                height: 360,
                crop: false
            }),
            "scale=640:360:flags=fast_bilinear"
        );
        assert!(
            scale_filter(OutputSize {
                width: 640,
                height: 360,
                crop: true
            })
            .contains("force_original_aspect_ratio=increase")
        );
    }

    #[test]
    fn reads_frames_and_detects_clean_eof() {
        let mut source = Cursor::new(vec![7u8; 10]);
        let mut buffer = [0u8; 4];
        assert_eq!(read_frame(&mut source, &mut buffer).unwrap(), 4);
        assert_eq!(buffer, [7; 4]);
        assert_eq!(read_frame(&mut source, &mut buffer).unwrap(), 4);
        assert_eq!(read_frame(&mut source, &mut buffer).unwrap(), 2);
        assert_eq!(read_frame(&mut source, &mut buffer).unwrap(), 0);
    }

    #[test]
    fn metadata_is_comparable() {
        let metadata = VideoMetadata {
            width: 2,
            height: 2,
            fps: 30.0,
        };
        assert_eq!(metadata.clone(), metadata);
    }
}
