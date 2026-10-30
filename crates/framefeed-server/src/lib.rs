//! `framefeed serve`: terminal video over HTTP, watchable with `curl`.
//!
//! The body of `GET /stream` is a paced sequence of escape sequences, not a
//! media container: the client terminal paints whatever bytes arrive. That is the
//! parrot.live model, but with real pixels and a quality knob.
//!
//! Deliberately dependency-free: a small hand-written HTTP/1.1 implementation is
//! enough for one endpoint, keeps the binary small, and makes the security
//! surface (request parsing, URL validation) visible and testable.

#![forbid(unsafe_code)]

mod http;
mod params;
mod source_guard;

pub use http::{ServerConfig, handle_connection, run};
pub use params::{BackendChoice, ParamError, StreamParams, parse_query, percent_decode};
pub use source_guard::{SourceError, is_public_ip, validate_source};
