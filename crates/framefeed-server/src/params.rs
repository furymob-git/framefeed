//! `/stream` query parameters.

use std::error::Error;
use std::fmt;

use framefeed_core::Fit;
use framefeed_proto::{Backend, Budget};
use framefeed_source::{Quality, QualityError};

/// Display backend selectable per request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    /// Truecolor half-blocks; works in any terminal and any `curl`.
    Ansi,
    /// Kitty graphics APC stream; the client must be a kitty-protocol terminal.
    Kitty,
    /// Sixel stream; Windows Terminal 1.22+, foot, WezTerm, Konsole, iTerm2.
    Sixel,
}

impl BackendChoice {
    /// Parses `ansi` or `kitty`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ansi" => Some(Self::Ansi),
            "kitty" => Some(Self::Kitty),
            "sixel" => Some(Self::Sixel),
            _ => None,
        }
    }

    /// Stable identifier used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ansi => "ansi",
            Self::Kitty => "kitty",
            Self::Sixel => "sixel",
        }
    }

    /// Performance envelope for this backend.
    #[must_use]
    pub const fn budget(self) -> Budget {
        match self {
            Self::Ansi => Backend::Ansi.budget(),
            Self::Kitty => Backend::Kitty.budget(),
            Self::Sixel => Backend::Sixel.budget(),
        }
    }
}

/// Validated request parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamParams {
    /// Source path or URL.
    pub source: String,
    /// Requested quality.
    pub quality: Quality,
    /// Display backend.
    pub backend: BackendChoice,
    /// Presentation rate cap, 1-60.
    pub fps: u8,
    /// Viewport columns, 20-300.
    pub cols: u16,
    /// Viewport rows, 5-100.
    pub rows: u16,
    /// Fit mode.
    pub fit: Fit,
}

/// Invalid query parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamError {
    /// `url` is missing or empty.
    MissingUrl,
    /// `backend` is not supported.
    UnknownBackend(String),
    /// `q` is not a valid quality.
    Quality(QualityError),
    /// `fit` is not supported.
    UnknownFit(String),
    /// A numeric parameter is out of range or not a number.
    Number {
        /// Parameter name.
        name: &'static str,
        /// Rejected value.
        value: String,
    },
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUrl => f.write_str("missing `url` parameter"),
            Self::UnknownBackend(value) => {
                write!(f, "unknown backend `{value}`; use ansi, kitty or sixel")
            }
            Self::Quality(error) => write!(f, "{error}"),
            Self::UnknownFit(value) => {
                write!(f, "unknown fit `{value}`; use contain, cover or stretch")
            }
            Self::Number { name, value } => write!(f, "`{name}` expects a number, got `{value}`"),
        }
    }
}

impl Error for ParamError {}

/// Parses and clamps `/stream` query parameters.
///
/// Defaults: `q=auto`, `backend=ansi`, `fps=30`, `cols=80`, `rows=24`, `fit=contain`.
///
/// `backend=ansi` is the default because it is the only backend that cannot corrupt
/// a terminal that lacks the protocol; ask for `sixel` or `kitty` explicitly.
///
/// # Errors
///
/// Returns [`ParamError`] for a missing URL, an unsupported backend/fit, an
/// invalid quality, or a non-numeric value.
pub fn parse_query(query: &str) -> Result<StreamParams, ParamError> {
    parse_stream_query(query, None)
}

/// Parses query parameters for a request whose source may come from the path.
///
/// `default_source` is used when the query has no `url` (the `/<name>` media
/// route); an explicit `url` still wins.
///
/// # Errors
///
/// Same as [`parse_query`]: a missing source, unsupported backend/fit, invalid
/// quality or non-numeric value.
pub fn parse_stream_query(
    query: &str,
    default_source: Option<String>,
) -> Result<StreamParams, ParamError> {
    let mut source = default_source;
    let mut quality = Quality::Auto;
    let mut backend = BackendChoice::Ansi;
    let mut fps = 30u8;
    let mut cols = 80u16;
    let mut rows = 24u16;
    let mut fit = Fit::Contain;

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(key);
        let value = percent_decode(value);
        match key.as_str() {
            "url" => source = Some(value),
            "q" | "quality" => quality = Quality::parse(&value).map_err(ParamError::Quality)?,
            "backend" => {
                backend = BackendChoice::parse(&value)
                    .ok_or_else(|| ParamError::UnknownBackend(value.clone()))?;
            }
            "fps" => {
                // Parse wide, then clamp: `fps=999` is out of range, not malformed.
                fps = u8::try_from(parse_number::<u32>("fps", &value)?.clamp(1, 60)).unwrap_or(30);
            }
            "cols" => {
                cols = u16::try_from(parse_number::<u32>("cols", &value)?.clamp(20, 300))
                    .unwrap_or(80);
            }
            "rows" => {
                rows =
                    u16::try_from(parse_number::<u32>("rows", &value)?.clamp(5, 100)).unwrap_or(24);
            }
            "fit" => {
                fit = match value.as_str() {
                    "contain" => Fit::Contain,
                    "cover" => Fit::Cover,
                    "stretch" => Fit::Stretch,
                    other => return Err(ParamError::UnknownFit(other.to_string())),
                };
            }
            _ => {}
        }
    }

    let source = source
        .filter(|value| !value.trim().is_empty())
        .ok_or(ParamError::MissingUrl)?;
    Ok(StreamParams {
        source,
        quality,
        backend,
        fps,
        cols,
        rows,
        fit,
    })
}

fn parse_number<T: std::str::FromStr>(name: &'static str, value: &str) -> Result<T, ParamError> {
    value.trim().parse().map_err(|_| ParamError::Number {
        name,
        value: value.to_string(),
    })
}

/// Decodes `%XX` and `+` in a query component. Invalid escapes are kept as-is.
#[must_use]
pub fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        decoded.push(high << 4 | low);
                        index += 3;
                    }
                    _ => {
                        decoded.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendChoice, ParamError, parse_query, percent_decode};
    use framefeed_core::Fit;
    use framefeed_source::Quality;

    #[test]
    fn parses_full_query() {
        let params = parse_query(
            "url=https%3A%2F%2Fexample.com%2Fv.m3u8&q=360p&backend=kitty&fps=24&cols=100&rows=30&fit=cover",
        )
        .unwrap();
        assert_eq!(params.source, "https://example.com/v.m3u8");
        assert_eq!(params.quality, Quality::Height(360));
        assert_eq!(params.backend, BackendChoice::Kitty);
        assert_eq!(params.fps, 24);
        assert_eq!(params.cols, 100);
        assert_eq!(params.rows, 30);
        assert_eq!(params.fit, Fit::Cover);
    }

    #[test]
    fn applies_defaults_and_clamps() {
        let params = parse_query("url=./clip.mp4&fps=999&cols=2&rows=999").unwrap();
        assert_eq!(params.quality, Quality::Auto);
        assert_eq!(params.backend, BackendChoice::Ansi);
        assert_eq!(params.fps, 60);
        assert_eq!(params.cols, 20);
        assert_eq!(params.rows, 100);
        assert_eq!(params.fit, Fit::Contain);
    }

    #[test]
    fn rejects_missing_url_and_unknown_values() {
        assert_eq!(parse_query("q=360p"), Err(ParamError::MissingUrl));
        assert_eq!(parse_query("url="), Err(ParamError::MissingUrl));
        assert!(matches!(
            parse_query("url=x&backend=iterm2"),
            Err(ParamError::UnknownBackend(_))
        ));
        assert!(matches!(
            parse_query("url=x&fit=zoom"),
            Err(ParamError::UnknownFit(_))
        ));
        assert!(matches!(
            parse_query("url=x&fps=abc"),
            Err(ParamError::Number { name: "fps", .. })
        ));
        assert!(matches!(
            parse_query("url=x&q=nope"),
            Err(ParamError::Quality(_))
        ));
    }

    #[test]
    fn percent_decoding_handles_plus_and_invalid_escapes() {
        assert_eq!(percent_decode("a+b%20c"), "a b c");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%4"), "%4");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn backend_names_are_stable() {
        assert_eq!(BackendChoice::Ansi.name(), "ansi");
        assert_eq!(BackendChoice::Kitty.name(), "kitty");
        assert_eq!(BackendChoice::Sixel.name(), "sixel");
        assert_eq!(BackendChoice::parse("KITTY"), Some(BackendChoice::Kitty));
        assert_eq!(BackendChoice::parse("sixel"), Some(BackendChoice::Sixel));
        assert_eq!(BackendChoice::parse("iterm2"), None);
    }
}
