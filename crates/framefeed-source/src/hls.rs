//! HLS master playlist parsing and variant selection.
//!
//! Scope: `#EXT-X-STREAM-INF` variants (bandwidth, resolution, codecs) plus URI
//! resolution. Renditions (`#EXT-X-MEDIA`), encryption and DASH are out of scope
//! for now; they are tracked in `docs/ROADMAP.md` (M3).

use std::error::Error;
use std::fmt;

use framefeed_core::Size;

const STREAM_INF: &str = "#EXT-X-STREAM-INF:";

/// One adaptive variant advertised by a master playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    /// Variant URI, exactly as written in the playlist (resolve with [`resolve_uri`]).
    pub uri: String,
    /// Declared peak bandwidth in bits per second.
    pub bandwidth: u64,
    /// Declared resolution, when present.
    pub resolution: Option<Size>,
    /// Declared codecs, in playlist order (`avc1.64001f`, `mp4a.40.2`, ...).
    pub codecs: Vec<String>,
}

/// A parsed HLS master playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasterPlaylist {
    /// Variants in playlist order.
    pub variants: Vec<Variant>,
}

/// Playlist parsing failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HlsError {
    /// The document has no `#EXT-X-STREAM-INF` tag (likely a media playlist).
    NotAMasterPlaylist,
    /// A `#EXT-X-STREAM-INF` tag was not followed by a URI line.
    MissingUri,
    /// No variant could be parsed.
    NoVariants,
    /// `BANDWIDTH` is mandatory on every variant.
    MissingBandwidth,
    /// An attribute value could not be parsed.
    MalformedAttribute(&'static str),
}

impl fmt::Display for HlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAMasterPlaylist => {
                f.write_str("not an HLS master playlist (no #EXT-X-STREAM-INF)")
            }
            Self::MissingUri => f.write_str("variant tag is not followed by a URI"),
            Self::NoVariants => f.write_str("playlist contains no variants"),
            Self::MissingBandwidth => f.write_str("variant is missing the BANDWIDTH attribute"),
            Self::MalformedAttribute(name) => write!(f, "invalid {name} attribute"),
        }
    }
}

impl Error for HlsError {}

/// Parses an HLS master playlist.
///
/// # Errors
///
/// See [`HlsError`]: media playlists, missing URIs/bandwidth, or unparsable
/// `RESOLUTION` values are rejected rather than guessed.
pub fn parse_master_playlist(text: &str) -> Result<MasterPlaylist, HlsError> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .peekable();
    let mut variants = Vec::new();
    let mut saw_variant_tag = false;

    while let Some(line) = lines.next() {
        let Some(raw_attributes) = line.strip_prefix(STREAM_INF) else {
            continue;
        };
        saw_variant_tag = true;

        let attributes = parse_attributes(raw_attributes);
        let uri = loop {
            match lines.next() {
                Some(candidate) if candidate.starts_with('#') => continue,
                Some(candidate) => break candidate,
                None => return Err(HlsError::MissingUri),
            }
        };

        let bandwidth = attribute(&attributes, "BANDWIDTH")
            .ok_or(HlsError::MissingBandwidth)?
            .parse::<u64>()
            .map_err(|_| HlsError::MalformedAttribute("BANDWIDTH"))?;
        let resolution = attribute(&attributes, "RESOLUTION")
            .map(parse_resolution)
            .transpose()?;
        let codecs = attribute(&attributes, "CODECS")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|codec| !codec.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        variants.push(Variant {
            uri: uri.to_string(),
            bandwidth,
            resolution,
            codecs,
        });
    }

    if !saw_variant_tag {
        return Err(HlsError::NotAMasterPlaylist);
    }
    if variants.is_empty() {
        return Err(HlsError::NoVariants);
    }
    Ok(MasterPlaylist { variants })
}

/// Chooses the cheapest variant that still matches `target_height`.
///
/// Picks the smallest resolution whose height is at least the target, breaking
/// ties on lower bandwidth. If every variant is smaller than the target, the
/// largest one wins. Variants without a declared resolution are ignored.
#[must_use]
pub fn select_variant(variants: &[Variant], target_height: u32) -> Option<&Variant> {
    let target = target_height.max(1);
    let mut best_above: Option<&Variant> = None;
    let mut largest: Option<&Variant> = None;

    for variant in variants {
        let Some(resolution) = variant.resolution else {
            continue;
        };
        let better_largest = largest.is_none_or(|current| {
            let current_height = current.resolution.map_or(0, |size| size.height);
            resolution.height > current_height
                || (resolution.height == current_height && variant.bandwidth < current.bandwidth)
        });
        if better_largest {
            largest = Some(variant);
        }
        if resolution.height >= target {
            let better_above = best_above.is_none_or(|current| {
                let current_height = current.resolution.map_or(0, |size| size.height);
                resolution.height < current_height
                    || (resolution.height == current_height
                        && variant.bandwidth < current.bandwidth)
            });
            if better_above {
                best_above = Some(variant);
            }
        }
    }

    best_above.or(largest)
}

/// Resolves a playlist URI against the playlist's own location.
///
/// Handles absolute URLs, protocol-relative URIs, root-relative paths and plain
/// relative paths (including local filesystem paths). Query strings and
/// fragments are treated as opaque parts of the URI, which is enough for HLS.
#[must_use]
pub fn resolve_uri(base: &str, uri: &str) -> String {
    if has_scheme(uri) {
        return uri.to_string();
    }
    if let Some(rest) = uri.strip_prefix("//") {
        let scheme = scheme(base).unwrap_or("https");
        return format!("{scheme}://{rest}");
    }
    if uri.starts_with('/') {
        return match origin(base) {
            Some(origin) => format!("{origin}{uri}"),
            None => uri.to_string(),
        };
    }
    match base.rsplit_once('/') {
        Some((directory, _)) if !directory.is_empty() => format!("{directory}/{uri}"),
        _ => uri.to_string(),
    }
}

fn has_scheme(uri: &str) -> bool {
    uri.find("://").is_some_and(|index| index > 0)
}

fn scheme(base: &str) -> Option<&str> {
    base.split_once("://").map(|(scheme, _)| scheme)
}

fn origin(base: &str) -> Option<&str> {
    let (_, rest) = base.split_once("://")?;
    let offset = base.len() - rest.len();
    let host_len = rest.find('/').unwrap_or(rest.len());
    Some(&base[..offset + host_len])
}

/// Parses `key=value` pairs, ignoring commas inside quoted values.
fn parse_attributes(input: &str) -> Vec<(&str, &str)> {
    let mut fields = Vec::new();
    let mut field_start = 0;
    let mut in_quotes = false;
    for (index, character) in input.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push((field_start, index));
                field_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    fields.push((field_start, input.len()));

    fields
        .into_iter()
        .filter_map(|(start, end)| {
            let field = input[start..end].trim();
            let (key, value) = field.split_once('=')?;
            Some((key.trim(), value.trim().trim_matches('"')))
        })
        .collect()
}

fn attribute<'a>(attributes: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}

fn parse_resolution(value: &str) -> Result<Size, HlsError> {
    let (width, height) = value
        .split_once(['x', 'X'])
        .ok_or(HlsError::MalformedAttribute("RESOLUTION"))?;
    let width = width
        .trim()
        .parse::<u32>()
        .map_err(|_| HlsError::MalformedAttribute("RESOLUTION"))?;
    let height = height
        .trim()
        .parse::<u32>()
        .map_err(|_| HlsError::MalformedAttribute("RESOLUTION"))?;
    if width == 0 || height == 0 {
        return Err(HlsError::MalformedAttribute("RESOLUTION"));
    }
    Ok(Size::new(width, height))
}

#[cfg(test)]
mod tests {
    use super::{
        HlsError, MasterPlaylist, Variant, parse_master_playlist, resolve_uri, select_variant,
    };
    use framefeed_core::Size;

    const MASTER: &str = "\
#EXTM3U
#EXT-X-VERSION:3
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"en\",URI=\"audio/en.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=5800000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,mp4a.40.2\"
v/1080p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2800000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\"
v/720p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1400000,RESOLUTION=854x480,CODECS=\"avc1.4d401f,mp4a.40.2\"
v/480p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS=\"avc1.42e01e,mp4a.40.2\"
v/360p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=400000,RESOLUTION=426x240
v/240p.m3u8
";

    fn variant(height: u32, bandwidth: u64, uri: &str) -> Variant {
        Variant {
            uri: uri.to_string(),
            bandwidth,
            resolution: Some(Size::new(height * 16 / 9, height)),
            codecs: Vec::new(),
        }
    }

    #[test]
    fn parses_every_variant_with_attributes() {
        let playlist = parse_master_playlist(MASTER).unwrap();
        assert_eq!(playlist.variants.len(), 5);

        let first = &playlist.variants[0];
        assert_eq!(first.uri, "v/1080p.m3u8");
        assert_eq!(first.bandwidth, 5_800_000);
        assert_eq!(first.resolution, Some(Size::new(1920, 1080)));
        assert_eq!(first.codecs, vec!["avc1.640028", "mp4a.40.2"]);

        let last = &playlist.variants[4];
        assert_eq!(last.bandwidth, 400_000);
        assert!(last.codecs.is_empty());
    }

    #[test]
    fn rejects_media_playlists() {
        let media = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n";
        assert_eq!(
            parse_master_playlist(media),
            Err(HlsError::NotAMasterPlaylist)
        );
    }

    #[test]
    fn rejects_variant_without_uri_or_bandwidth() {
        assert_eq!(
            parse_master_playlist("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n"),
            Err(HlsError::MissingUri)
        );
        assert_eq!(
            parse_master_playlist("#EXTM3U\n#EXT-X-STREAM-INF:RESOLUTION=640x360\nv.m3u8\n"),
            Err(HlsError::MissingBandwidth)
        );
        assert_eq!(
            parse_master_playlist(
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=wide\nv.m3u8\n"
            ),
            Err(HlsError::MalformedAttribute("RESOLUTION"))
        );
    }

    #[test]
    fn selects_smallest_variant_above_target() {
        let playlist = parse_master_playlist(MASTER).unwrap();
        let chosen = select_variant(&playlist.variants, 360).unwrap();
        assert_eq!(chosen.uri, "v/360p.m3u8");

        let chosen = select_variant(&playlist.variants, 300).unwrap();
        assert_eq!(chosen.uri, "v/360p.m3u8");

        let chosen = select_variant(&playlist.variants, 720).unwrap();
        assert_eq!(chosen.uri, "v/720p.m3u8");
    }

    #[test]
    fn falls_back_to_largest_variant_when_target_is_huge() {
        let playlist = parse_master_playlist(MASTER).unwrap();
        let chosen = select_variant(&playlist.variants, 2160).unwrap();
        assert_eq!(chosen.uri, "v/1080p.m3u8");
    }

    #[test]
    fn breaks_ties_on_bandwidth_and_ignores_unknown_resolutions() {
        let variants = vec![
            variant(720, 3_000_000, "expensive.m3u8"),
            variant(720, 2_400_000, "cheap.m3u8"),
            Variant {
                uri: "unknown.m3u8".into(),
                bandwidth: 1,
                resolution: None,
                codecs: Vec::new(),
            },
        ];
        assert_eq!(
            select_variant(&variants, 720).unwrap().uri,
            "cheap.m3u8",
            "same height must prefer lower bandwidth"
        );
        assert!(select_variant(&variants[2..], 720).is_none());
        assert!(select_variant(&[], 720).is_none());
    }

    #[test]
    fn resolves_relative_root_relative_and_absolute_uris() {
        let base = "https://cdn.example.com/live/master.m3u8";
        assert_eq!(
            resolve_uri(base, "v/360p.m3u8"),
            "https://cdn.example.com/live/v/360p.m3u8"
        );
        assert_eq!(
            resolve_uri(base, "/root/360p.m3u8"),
            "https://cdn.example.com/root/360p.m3u8"
        );
        assert_eq!(
            resolve_uri(base, "//other.example.com/x.m3u8"),
            "https://other.example.com/x.m3u8"
        );
        assert_eq!(
            resolve_uri(base, "https://third.example/y.m3u8"),
            "https://third.example/y.m3u8"
        );
        assert_eq!(
            resolve_uri("fixtures/master.m3u8", "360p.m3u8"),
            "fixtures/360p.m3u8"
        );
        assert_eq!(resolve_uri("master.m3u8", "360p.m3u8"), "360p.m3u8");
    }

    #[test]
    fn master_playlist_equality_is_structural() {
        let playlist = parse_master_playlist(MASTER).unwrap();
        assert_eq!(
            playlist,
            MasterPlaylist {
                variants: playlist.variants.clone()
            }
        );
    }
}
