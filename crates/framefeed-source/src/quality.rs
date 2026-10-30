//! The `--quality` value: a height target, or `auto`.

use std::error::Error;
use std::fmt;

/// Highest height accepted by `--quality` (8K UHD), a sanity bound, not a promise.
const MAX_HEIGHT: u32 = 4320;

/// Requested output quality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    /// Derive the target from the terminal geometry and backend budget.
    #[default]
    Auto,
    /// Target height in pixels; the raster never exceeds it.
    Height(u32),
}

impl Quality {
    /// Parses `auto`, `360`, `360p` or `360P`.
    ///
    /// # Errors
    ///
    /// Returns [`QualityError`] for empty input, non-numeric values, suffixes
    /// other than `p`, zero, or heights above 8K.
    pub fn parse(value: &str) -> Result<Self, QualityError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(QualityError::Empty);
        }
        if value.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        let digits = value.strip_suffix(['p', 'P']).unwrap_or(value).trim();
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(QualityError::NotAHeight(value.to_string()));
        }
        let height: u32 = digits
            .parse()
            .map_err(|_| QualityError::NotAHeight(value.to_string()))?;
        if height == 0 {
            return Err(QualityError::NotAHeight(value.to_string()));
        }
        if height > MAX_HEIGHT {
            return Err(QualityError::TooHigh(height));
        }
        Ok(Self::Height(height))
    }

    /// Resolves to a pixel height, using `auto_height` for [`Quality::Auto`].
    #[must_use]
    pub const fn target_height(self, auto_height: u32) -> u32 {
        match self {
            Self::Auto => auto_height,
            Self::Height(height) => height,
        }
    }
}

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Height(height) => write!(f, "{height}p"),
        }
    }
}

/// Why a `--quality` value was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualityError {
    /// The value was empty or whitespace.
    Empty,
    /// The value is not `auto` or `<height>[p]`.
    NotAHeight(String),
    /// The requested height exceeds 8K.
    TooHigh(u32),
}

impl fmt::Display for QualityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("quality cannot be empty; use e.g. 360p or auto"),
            Self::NotAHeight(value) => {
                write!(f, "`{value}` is not a quality; use e.g. 360p or auto")
            }
            Self::TooHigh(height) => write!(f, "{height}p is above the 4320p limit"),
        }
    }
}

impl Error for QualityError {}

#[cfg(test)]
mod tests {
    use super::{Quality, QualityError};

    #[test]
    fn parses_common_forms() {
        assert_eq!(Quality::parse("144p").unwrap(), Quality::Height(144));
        assert_eq!(Quality::parse("360P").unwrap(), Quality::Height(360));
        assert_eq!(Quality::parse("1080").unwrap(), Quality::Height(1080));
        assert_eq!(Quality::parse(" auto ").unwrap(), Quality::Auto);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Quality::parse(""), Err(QualityError::Empty));
        assert!(matches!(
            Quality::parse("hd"),
            Err(QualityError::NotAHeight(_))
        ));
        assert!(matches!(
            Quality::parse("1080i"),
            Err(QualityError::NotAHeight(_))
        ));
        assert!(matches!(
            Quality::parse("0p"),
            Err(QualityError::NotAHeight(_))
        ));
        assert_eq!(Quality::parse("9999p"), Err(QualityError::TooHigh(9999)));
    }

    #[test]
    fn auto_uses_the_terminal_height() {
        assert_eq!(Quality::Auto.target_height(384), 384);
        assert_eq!(Quality::Height(240).target_height(384), 240);
    }

    #[test]
    fn display_roundtrips() {
        assert_eq!(Quality::Auto.to_string(), "auto");
        assert_eq!(Quality::Height(480).to_string(), "480p");
    }
}
