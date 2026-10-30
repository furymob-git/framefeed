# Roadmap

Milestones are shippable slices, each with exit criteria. A milestone is done when its
criteria are met on Linux, macOS **and** Windows, with the CI job green.

## M0 — Design & repo hygiene ✅

- [x] Architecture, protocol research, performance budgets, ADRs
- [x] Repo skeleton, lint policy, CI (fmt, clippy, test, msrv, deny, docs)
- [x] Contribution process, issue/PR templates, security policy
- [x] CLI contract (`play`, `serve`, `doctor`) documented, stub compiles

**Exit criteria:** `cargo build`, `cargo clippy -D warnings`, `cargo fmt --check` and
`cargo test` pass on all three platforms; docs reviewed.

## M1 — First light (kitty + local file)

- [x] Split crates: `framefeed-core`, `framefeed-proto`, `framefeed-source`
- [ ] Split out `framefeed-decode` together with the ffmpeg sidecar
- [ ] ffmpeg sidecar decoder: probe metadata, pipe `rgba` frames, seek, EOF
- [ ] Renderer: SIMD downscale + fit modes + cell-pixel geometry
- [x] Kitty encoder: image id, placement, `C=1`, `o=z`, chunked direct transmission
- [x] Render planning: viewport geometry, fit modes, backend/quality height cap
- [x] HLS master playlist parsing and `--quality` variant selection (library)
- [x] `framefeed doctor` (hint-based) and `framefeed probe` (playlist inspection)
- [ ] Pacer: PTS clock, drop-oldest channel, `--fps`
- [ ] Terminal guard: raw mode, alternate screen, cursor, panic-safe restore
- [ ] `framefeed play ./clip.mp4` works; `--stats` reports fps/dropped/bytes
- [ ] Benchmarks: criterion for scale+encode, hyperfine for startup/first frame

**Exit criteria:** 60 fps at 120×34 cells on the reference machine with 0 dropped
frames on a 30 s 1080p fixture; clean exit restores the terminal; byte-exact golden
tests for the kitty encoder.

## M2 — Universal (sixel, ANSI, detection)

- [ ] Sixel encoder with 240-color cube + RLE (adaptive palette behind a flag)
- [ ] ANSI half-block and ASCII backends
- [ ] Capability detection (hints → probes → fallback) with timeouts
- [ ] `framefeed doctor` prints the resolved matrix for the current terminal
- [ ] Resize handling (SIGWINCH / console events) with debounce
- [ ] tmux passthrough and SSH-aware budgets

**Exit criteria:** the same command plays on kitty, WezTerm, Windows Terminal 1.22+,
iTerm2, foot and Alacritty without manual flags; `doctor` matches the README matrix;
sixel holds 30 fps at 800×480.

## M3 — Streaming (HLS, quality)

- [ ] `framefeed-source`: local file, HTTP progressive, HLS master/media playlists
- [ ] Variant selection: `--quality 144p…1080p|auto` → smallest adequate variant
- [ ] Buffering, timeouts, retries, reconnect for live streams
- [ ] `yt-dlp` adapter for sites that need it (`--via yt-dlp`, optional binary)
- [ ] `--probe` prints source metadata and chosen variant without playing

**Exit criteria:** a 10-minute HLS stream plays with bounded latency drift and no
stall on a 20 Mbit/s link; `--quality 144p` measurably downloads less than 1080p
(report bytes/sec in `--stats`); reconnection recovers from a dropped connection.

## M4 — Real player (audio, seek, controls)

- [ ] Audio decode (symphonia) + output (cpal), audio clock as master
- [ ] Pause/resume, seek ±5 s/±30 s, frame step, quit
- [ ] A/V drift < 50 ms over 10 minutes; `--no-audio` for silent playback
- [ ] Subtitles: out of scope for 1.0 (documented, issue open)

**Exit criteria:** a 10-minute mp4 plays in sync in a kitty terminal while the terminal
remains interactive after exit; seeking is < 250 ms to first frame.

## M5 — `serve` (curl mode)

- [ ] `framefeed serve`: `GET /stream?url=<src>&q=<quality>&backend=<name>`
- [ ] Frame-paced ANSI/ASCII/sixel output with cursor-home repaint (`curl -N` friendly)
- [ ] SSRF guard (scheme allowlist, no link-local/loopback targets by default),
      per-IP rate limits, max concurrent streams, idle timeout
- [ ] `GET /healthz`, structured logs; deployment notes for a small VPS

**Exit criteria:** `curl -N 'http://localhost:8080/stream?url=./clip.mp4&q=144p'`
animates in a plain terminal; fuzzed URL handling produces no panics or SSRF bypass.

## M6 — Polish & packaging

- [ ] Kitty shared-memory transport (`t=s`) and temp-file transport, benchmarked
- [ ] Opt-in animated/delta-frame mode for kitty (`a=f`, partial rectangles, `a=c`)
- [ ] `libav` in-process decoder behind a feature flag
- [ ] Release pipeline (`cargo-dist`), Homebrew/Scoop/Winget, shell completions
- [ ] Docs site, demo recordings (asciinema/GIF), published benchmark table

**Exit criteria:** installable in one command on all three platforms; published
benchmarks reproducible from a documented command.

## Explicitly out of scope for 1.0

- GUI/TUI chrome beyond a one-line status bar
- Video editing, transcoding, recording, screenshots of the terminal
- Embedding as a library in editors; plugin APIs
- DRM, Widevine, encrypted HLS
- GPU-accelerated decoding/rendering on our side

## How milestones are tracked

Every milestone gets a GitHub milestone; every task a `good first issue` or `help
wanted` label where appropriate. A PR references the milestone in its description.
Scope changes require an ADR or a roadmap edit in the same PR.
