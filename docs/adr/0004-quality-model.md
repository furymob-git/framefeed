# 0004 — Quality as render budget plus source variant

- Status: Accepted
- Date: 2026-10-30

## Context

Users ask for `--quality 144p` and expect two things at once: a smaller picture and
less bandwidth. Naively, quality only affects the output raster — we would still
download a 1080p stream and downscale it, which is wasteful on a phone tether and
pointless on a 20-column terminal.

Adaptive sources expose variants (HLS master playlist `RESOLUTION`/`BANDWIDTH`), so the
choice can happen before any bytes of video are fetched.

## Decision

`--quality` is a single knob that drives both:

1. **Render budget**: the output raster height never exceeds the requested height, also
   capped by the backend budget.
2. **Source selection**: for adaptive sources, pick the smallest variant whose height is
   ≥ the target, breaking ties by bandwidth; fall back to the largest available.

`auto` derives the target from terminal geometry (cell size × rows) and re-evaluates on
resize with hysteresis.

## Consequences

- Bandwidth scales with what the terminal can actually display; `--stats` reports both
  the chosen variant and bytes/sec so the promise is testable.
- The source layer must parse manifests itself (not delegate variant choice to ffmpeg),
  because we need the decision and its rationale for `--probe`.
- Live streams can only switch at segment boundaries; the policy is documented in
  `docs/STREAMING.md`.
- Unsupported codecs in a cheap variant may force the next variant up; the probe
  reports this instead of silently falling back.

## Alternatives considered

- **Raster-only quality**: simplest, but does not save bandwidth.
- **Bitrate-only quality** (`--max-bitrate`): useful as a secondary cap, but opaque to
  users and unrelated to the terminal's pixel budget.
- **Delegate to ffmpeg's HLS demuxer**: it auto-selects by bandwidth/height heuristics we
  cannot inspect or reproduce, and it cannot tell us which variant it chose before
  playback.
