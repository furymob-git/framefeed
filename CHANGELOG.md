# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Licensed under MIT only (previously MIT OR Apache-2.0), with the license text in
  `LICENSE` so GitHub detects it.
- Release builds unwind instead of aborting, so the terminal guard restores the
  screen and cursor on panic.

### Added

- Sixel backend: 240-colour palette (6×6×6 cube + 24-step grey ramp), per-colour
  run-length encoding and band repaint. Windows Terminal 1.22+, foot, WezTerm,
  Konsole and iTerm2 now draw real pixels; `--backend auto` prefers Kitty, then
  Sixel, then ANSI, and `serve` accepts `backend=sixel`.
- Playback now sizes itself from the real terminal (falling back to `COLUMNS` /
  `LINES`), and Windows Terminal's Sixel path uses its 10×20 px cell grid so the
  raster matches the window instead of being resampled. The Sixel budget rises to
  1080 px tall to reach full-window height.
- Sixel frames home the cursor before each image, so frames replace each other
  instead of stacking and scrolling; `play --stats` reports the cell grid used and
  `FRAMEFEED_SIZE=COLSxROWS` overrides the measured geometry.

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
- `framefeed play`: decodes a local file or URL through the ffmpeg sidecar, plans the
  raster from the viewport and `--quality`, paces presentation, and paints with the
  Kitty graphics protocol or ANSI half-blocks. Includes `--fps`, `--fit`,
  `--backend`, `--stats`, and Ctrl-C-safe terminal cleanup.
- `framefeed serve`: `GET /stream?url=&q=&backend=&fps=&cols=&rows=&fit=` streams
  escape sequences for `curl -N`, with `GET /healthz`, an SSRF guard (scheme
  allowlist, private-address rejection) and a concurrent-stream limit.
- `framefeed-decode`: ffmpeg/ffprobe sidecar (metadata probe, raw RGBA pipe, clean
  EOF, kill-on-drop) with `FRAMEFEED_FFMPEG` / `FRAMEFEED_FFPROBE` overrides.
- `framefeed-server`: dependency-free HTTP/1.1 implementation for the stream endpoint.
- ANSI truecolor half-block encoder, fps-based pacer with drop accounting, and a
  terminal guard for the alternate screen and cursor.
- First measured playback numbers (local 30 fps clip, public HLS stream) in
  `docs/PERFORMANCE.md`, including the pacer bug they uncovered.

[Unreleased]: https://github.com/framefeed/framefeed/commits/main
