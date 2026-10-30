# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Licensed under MIT only (previously MIT OR Apache-2.0), with the license text in
  `LICENSE` so GitHub detects it.

### Added

- Repository bootstrap: workspace, lint policy, CI (fmt, clippy, test matrix, MSRV,
  `cargo-deny`, rustdoc), contribution and security policies, issue/PR templates.
- Architecture documentation: pipeline and threading model, protocol comparison
  (Kitty, Sixel, iTerm2, ANSI), performance budgets, streaming and quality model.
- ADRs 0001–0004: Rust, ffmpeg sidecar decoder, byte-oriented backend abstraction,
  quality semantics.
- CLI skeleton with the `play` / `serve` / `doctor` contract (not implemented).
- `framefeed doctor`: hint-based backend detection, per-backend budgets, viewport
  and raster geometry, multiplexer warnings.
- `framefeed probe`: parses a local HLS master playlist, lists variants and previews
  the `--quality` selection.
- `framefeed-core`: validated frame/raster containers, render planning with
  contain/cover/stretch fit modes and height caps.
- `framefeed-proto`: Kitty graphics encoder (image id reuse, placement, `C=1`,
  zlib payloads, 4096-byte chunking, cleanup), per-backend budgets, hint-based
  detection, and byte-exact encoder tests.
- `framefeed-source`: `--quality` parsing, HLS master playlist parser, variant
  selection (smallest adequate, bandwidth tie-break) and URI resolution.

[Unreleased]: https://github.com/framefeed/framefeed/commits/main
