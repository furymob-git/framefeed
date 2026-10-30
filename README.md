# framefeed

**Video in your terminal. Pixels, not ASCII soup.**

![status](https://img.shields.io/badge/status-design%20phase-orange)
[![CI](https://github.com/framefeed/framefeed/actions/workflows/ci.yml/badge.svg)](https://github.com/framefeed/framefeed/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT-blue)](#license)

> **Status: alpha.** `framefeed play` decodes local files and network streams and
> paints them with the Kitty graphics protocol, Sixel or truecolor ANSI half-blocks;
> `framefeed serve` streams terminal video over HTTP for `curl`. Audio and iTerm2
> output are still missing — see [docs/ROADMAP.md](docs/ROADMAP.md).
>
> Requires `ffmpeg` and `ffprobe` on `PATH` (or `FRAMEFEED_FFMPEG` /
> `FRAMEFEED_FFPROBE`).

framefeed is a terminal video player: it decodes an `.mp4`/`.webm`/`.mkv` file or a
network stream and paints the frames as real pixels inside your terminal, using the
best graphics protocol the terminal supports — Kitty graphics, Sixel, iTerm2 inline
images, or a truecolor half-block fallback that works anywhere.

It is built for two things the existing tools do not combine well: **low-latency local
playback at a stable frame rate**, and **streaming-first quality negotiation** (pick the
cheapest source variant that still looks right at your terminal's actual size, e.g.
`--quality 360p` or `?q=144p` over HTTP).

---

## Why another one?

| | Terminal output | Local files | Network / HLS | Quality / variant selection | Scriptable + tested |
|---|---|---|---|---|---|
| `mpv --vo=kitty` | kitty | yes | yes | via mpv | no |
| `timg` | kitty / sixel / iterm2 | images + short video | no | basic | partially |
| `notcurses` (`ncplayer`) | kitty / sixel | yes | no | no | C library |
| `tplay`, ascii players | ANSI/ASCII | yes | yes | no | varies |
| **framefeed** | kitty / sixel / ANSI (iTerm2, ASCII planned) | yes | yes | **first-class** | **yes** |

The niche is narrow and deliberate: a small, boring, well-tested core with a
protocol-pure encoder layer, benchmarked end-to-end, and a CLI that is pleasant to
script. If you just want to watch a movie, `mpv` is great. If you want a terminal
video *pipeline* you can build on, that is framefeed.

## Terminal support

Video means "can repaint fast enough to look like motion", not just "can show a PNG".

| Terminal | Graphics protocol | Video-ready | Notes |
|---|---|---|---|
| kitty | Kitty graphics | ✅ best | Native animation + shared-memory transport |
| WezTerm | Kitty graphics (opt-in), Sixel, iTerm2 | ✅ best on Windows | Set `enable_kitty_graphics = true` |
| Ghostty | Kitty graphics | ✅ | |
| Konsole | Kitty graphics, Sixel | ✅ | |
| iTerm2 | iTerm2 inline images, Sixel | ✅ | Full re-upload per frame; higher cost |
| Windows Terminal ≥ 1.22 | Sixel | ✅ | Kitty protocol not supported; raster matched to its 10×20 px grid |
| foot, mintty, Rio, Contour, mlterm | Sixel | ✅ | |
| Alacritty | none | ❌ | ANSI/ASCII fallback only |
| tmux / screen | passthrough | ⚠️ partial | `set -g allow-passthrough on`; Sixel needs tmux built with `--enable-sixel` |

`framefeed doctor` will print this table resolved for *your* terminal (Milestone M2).

> On Windows, kitty itself does not run natively: use WezTerm (Kitty graphics) or
> Windows Terminal 1.22+ (Sixel). This is the primary development platform for the
> project, so it is tested first, not last.
>
> framefeed drives **Kitty graphics**, **Sixel** and **ANSI half-blocks** today, so
> Windows Terminal 1.22+ draws real pixels through Sixel (`auto` picks it). On
> Windows Terminal the raster is planned on its 10×20 px Sixel grid and sized from
> the real window, so the picture is not upscaled. The iTerm2 encoder is the only
> display backend still missing.

## Usage

```console
$ framefeed play ./clip.mp4
$ framefeed play ./clip.webm --quality 480p --fps 30
$ framefeed play https://example.com/master.m3u8 --quality 720p --stats
$ framefeed play ./clip.mp4 --backend sixel --fit cover
$ framefeed play ./clip.mp4 --backend ansi
$ framefeed doctor
$ framefeed serve --allow-local --listen 127.0.0.1:8080
# then, in any terminal:
$ curl -N 'http://127.0.0.1:8080/stream?url=./clip.mp4&q=144p'
$ curl -N 'http://127.0.0.1:8080/sintel?q=480p&backend=sixel'   # bundled clip
```

`GET /` lists the bundled clips; `GET /<name>` plays one and takes the same optional
parameters (`q`, `backend`, `fps`, `cols`, `rows`, `fit`).

`--backend auto` prefers Kitty graphics, then Sixel, then ANSI half-blocks; force
one with `--backend kitty|sixel|ansi`. `--quality auto` (the default) sizes the
raster from the real terminal dimensions, so the video fills the window;
`FRAMEFEED_SIZE=COLSxROWS` forces the geometry when it cannot be measured.
Playback is video-only for now: audio and seeking are M4.

Quality is a *rendering* budget: `--quality 144p` caps the raster at 144 px tall, so
the decoder, encoder and pipe all move fewer pixels. The library also picks the
cheapest adequate HLS variant from a master playlist (`framefeed probe` shows the
decision), but the play path still hands the URL to ffmpeg, which applies its own
variant choice — wiring ours into playback is M3. Details:
[docs/STREAMING.md](docs/STREAMING.md).

## Design principles

1. **Degrade, never fail.** No graphics support must still produce a watchable
   picture (ANSI half-blocks), not an error.
2. **Budget-driven.** Every backend has a measured per-frame byte and CPU budget.
   Features that blow the budget are opt-in, not default.
3. **Drop frames, never lag.** Backpressure is handled by dropping the oldest frame
   in the queue, never by blocking the decoder.
4. **Protocol-pure encoders.** Escape-sequence writers are pure functions over
   buffers, byte-exact golden-tested, and `#![forbid(unsafe_code)]`.
5. **Streaming first.** Local files are the easy case; HLS, live streams, reconnects
   and variant selection shape the architecture from day one.
6. **Boring core.** No clever ownership tricks in the hot path, no global state, one
   obvious place where each decision lives.

## Architecture at a glance

```text
 source ──► decode ──► scale/fit ──► optional quantize/dither ──► encode ──► write
 (file,     (ffmpeg    (fast_image_    (sixel palette)            (kitty /    (buffered
  http,      sidecar     resize,                                 sixel /     tty writer,
  hls)       or libav)   SIMD)                                    ansi)       one syscall)
                            ▲                                        ▲
                            └──────────── Pacer (PTS clock, frame drop) ┘
```

Bounded channels between stages, drop-oldest on overflow, one reusable frame buffer
per stage, zero allocations in the steady state. Full rationale, threading model and
invariants: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Repository layout

```text
crates/
  framefeed-cli/      # binary: play / serve / doctor / probe
  framefeed-core/     # frames, render planning, pacer (pure, no dependencies)
  framefeed-proto/    # budgets, detection, kitty + ansi encoders, terminal guard
  framefeed-decode/   # ffmpeg/ffprobe sidecar decoder
  framefeed-source/   # quality model, HLS master playlist, variant selection
  framefeed-server/   # /stream and /healthz over a hand-written HTTP/1.1 server
videos/
  demo/               # committed, CC-BY demo clips served at /<name>
  ...                 # your own media, git-ignored (README kept)
docs/
  ARCHITECTURE.md     # pipeline, threading, invariants, portability
  PROTOCOLS.md        # kitty vs sixel vs iTerm2 vs ANSI: costs and detection
  PERFORMANCE.md      # targets, budgets, benchmark methodology, regression policy
  STREAMING.md        # sources, variant selection, `serve` API and abuse limits
  ROADMAP.md          # milestones and exit criteria
  adr/                # architecture decision records
```

The pipeline is wired end to end: `play` and `serve` both decode, plan, pace and
encode. Every crate has a real API and tests.

## Roadmap

M0 design & repo hygiene → M1 first light (Kitty, local file) → M2 universal
(Sixel + ANSI + detection) → M3 streaming (HLS, quality) → M4 real player (audio,
seek, controls) → M5 `serve` (curl mode) → M6 polish & packaging.
Exit criteria for each: [docs/ROADMAP.md](docs/ROADMAP.md).

## Deploy the server

`framefeed serve` ships as a container image with ffmpeg baked in. Tag a release
and the [container workflow](.github/workflows/deploy.yml) publishes
`ghcr.io/<owner>/<repo>`:

```console
$ docker run --rm -p 8080:8080 ghcr.io/<owner>/<repo>:latest
$ curl 'http://127.0.0.1:8080/healthz'
ok
$ curl -N 'http://127.0.0.1:8080/big-buck-bunny?q=480p&backend=sixel'
```

The image serves public `http(s)` sources (`url=` on `/stream`) **and** six CC-BY
Blender Open Movie excerpts bundled at [`videos/demo/`](videos/demo/CREDITS.md),
reachable by bare name — `curl -N http://127.0.0.1:8080/sintel`, with `GET /` listing
them.

Named clips are resolved inside one directory (no paths, no traversal), so
`--media` stays enabled without `--allow-local`, behind the same SSRF guard as the
binary. Hosting, limits and every query parameter: [docs/DEPLOY.md](docs/DEPLOY.md).

Drop your own clips in [`videos/`](videos/README.md) — personal media there is
git-ignored, and `scripts/play.bat` picks the first one when no path is given.

## Contributing

Contributions go through pull requests, never direct pushes to `main`. Start with
[CONTRIBUTING.md](CONTRIBUTING.md); large changes need an issue/RFC first.
By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
Security issues: [SECURITY.md](SECURITY.md), never a public issue.

## License

Released under the [MIT license](LICENSE). Unless you explicitly state otherwise,
any contribution intentionally submitted for inclusion in this project shall be
licensed under the same terms, without any additional conditions.
