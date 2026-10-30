//! Source validation: the SSRF surface of `serve`.
//!
//! We only accept http(s) URLs and local paths (opt-in). For URLs we resolve the
//! host and reject any address in a private, loopback, link-local, CGNAT, ULA or
//! multicast range. This blocks the obvious "make the server fetch its own cloud
//! metadata endpoint" attack.
//!
//! Known residual risk: a *public* playlist may reference private segment URLs,
//! which ffmpeg would then fetch. Mitigating that needs a demuxer-level allowlist;
//! it is tracked in `docs/STREAMING.md` and `docs/ROADMAP.md` (M5 hardening).

use std::error::Error;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Why a source was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    /// Local paths are disabled (`--allow-local` not passed).
    LocalNotAllowed,
    /// Scheme other than http/https (including `file://`).
    UnsupportedScheme(String),
    /// The URL has no resolvable host.
    Unresolvable(String),
    /// The host resolves to a non-public address.
    PrivateAddress(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalNotAllowed => {
                f.write_str("local paths are disabled; start the server with --allow-local")
            }
            Self::UnsupportedScheme(scheme) => {
                write!(f, "unsupported URL scheme `{scheme}`; only http and https")
            }
            Self::Unresolvable(host) => write!(f, "cannot resolve host `{host}`"),
            Self::PrivateAddress(address) => {
                write!(f, "refusing to fetch non-public address {address}")
            }
        }
    }
}

impl Error for SourceError {}

/// Validates a requested source before any process is spawned.
///
/// # Errors
///
/// See [`SourceError`]: local paths without opt-in, non-http schemes, unresolvable
/// hosts, and private/loopback/link-local addresses.
pub fn validate_source(raw: &str, allow_local: bool) -> Result<(), SourceError> {
    let source = raw.trim();
    if source.is_empty() {
        return Err(SourceError::Unresolvable(String::new()));
    }
    let has_http = source.starts_with("http://") || source.starts_with("https://");
    if !has_http && source.contains("://") {
        let scheme = source.split_once("://").map_or("", |(scheme, _)| scheme);
        return Err(SourceError::UnsupportedScheme(scheme.to_string()));
    }
    if !has_http {
        return if allow_local {
            Ok(())
        } else {
            Err(SourceError::LocalNotAllowed)
        };
    }

    let (host, port) =
        host_port(source).ok_or_else(|| SourceError::Unresolvable(source.to_string()))?;
    let mut addresses = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| SourceError::Unresolvable(host.clone()))?;
    let mut any = false;
    for address in addresses.by_ref() {
        any = true;
        if !is_public_ip(address.ip()) {
            return Err(SourceError::PrivateAddress(address.ip().to_string()));
        }
    }
    if !any {
        return Err(SourceError::Unresolvable(host));
    }
    Ok(())
}

/// True for addresses that are routable on the public internet.
#[must_use]
pub fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let cgnat = octets[0] == 100 && (64..=127).contains(&octets[1]);
    let benchmark = octets[0] == 198 && (octets[1] == 18 || octets[1] == 19);
    let reserved = octets[0] == 0;
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || cgnat
        || benchmark
        || reserved)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_v4(mapped);
    }
    let segments = ip.segments();
    let unique_local = segments[0] & 0xfe00 == 0xfc00;
    let link_local = segments[0] & 0xffc0 == 0xfe80;
    !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || unique_local || link_local)
}

/// Extracts `(host, port)` from an http(s) URL.
fn host_port(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let default_port = if scheme.eq_ignore_ascii_case("https") {
        443
    } else {
        80
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;

    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = tail
            .strip_prefix(':')
            .and_then(|port| port.parse().ok())
            .unwrap_or(default_port);
        return Some((host.to_string(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port))
            if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            Some((host.to_string(), port.parse().ok()?))
        }
        _ => Some((authority.to_string(), default_port)),
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceError, is_public_ip, validate_source};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    fn v4(address: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(address))
    }

    #[test]
    fn rejects_private_loopback_and_link_local_addresses() {
        for address in [
            [127, 0, 0, 1],
            [10, 1, 2, 3],
            [172, 16, 0, 1],
            [192, 168, 1, 1],
            [169, 254, 1, 1],
            [100, 64, 0, 1],
            [0, 0, 0, 0],
            [224, 0, 0, 1],
        ] {
            assert!(!is_public_ip(v4(address)), "{address:?} must be refused");
        }
    }

    #[test]
    fn accepts_public_addresses() {
        assert!(is_public_ip(v4([8, 8, 8, 8])));
        assert!(is_public_ip(v4([1, 1, 1, 1])));
        assert!(is_public_ip(IpAddr::V6(
            "2001:4860:4860::8888".parse::<Ipv6Addr>().unwrap()
        )));
    }

    #[test]
    fn rejects_ipv6_local_ranges() {
        for address in ["::1", "fe80::1", "fd00::1", "ff02::1", "::"] {
            let ip: IpAddr = address.parse().unwrap();
            assert!(!is_public_ip(ip), "{address} must be refused");
        }
    }

    #[test]
    fn blocks_loopback_urls_without_dns() {
        assert_eq!(
            validate_source("http://127.0.0.1:8080/internal", false),
            Err(SourceError::PrivateAddress("127.0.0.1".to_string()))
        );
        assert_eq!(
            validate_source("https://[::1]/x", false),
            Err(SourceError::PrivateAddress("::1".to_string()))
        );
    }

    #[test]
    fn local_paths_require_opt_in() {
        assert_eq!(
            validate_source("./clip.mp4", false),
            Err(SourceError::LocalNotAllowed)
        );
        assert_eq!(validate_source("./clip.mp4", true), Ok(()));
        assert_eq!(
            validate_source("file:///etc/passwd", true),
            Err(SourceError::UnsupportedScheme("file".to_string()))
        );
        assert_eq!(
            validate_source("  ", true),
            Err(SourceError::Unresolvable(String::new()))
        );
    }

    #[test]
    fn parses_host_and_port_from_authority_forms() {
        assert_eq!(
            validate_source("http://[::1]:9000/x", true),
            Err(SourceError::PrivateAddress("::1".to_string()))
        );
        assert!(matches!(
            validate_source("https://user:pass@127.0.0.1/x", true),
            Err(SourceError::PrivateAddress(_))
        ));
    }
}
