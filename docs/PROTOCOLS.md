# Display protocols

What we can actually draw with, what it costs, and how we choose. All byte figures
below are **budget estimates** for a 160×45 cell viewport with 8×16 px cells
(1280×720 raster); measured numbers land in `PERFORMANCE.md` with the M1 benchmarks.

## 1. Comparison

| Protocol | Terminals | Update model | Typical bytes/frame | Achievable fps (est.) | Alpha | Notes |
|---|---|---|---|---|---|---|
| **Kitty graphics** | kitty, WezTerm*, Ghostty, Konsole, iTerm2, Warp, xterm.js | Reuse image id + placement; partial frames / animation | 3.7 MB RGBA raw → ~1.0–2.0 MB zlib; shared memory path ≈ raw, no base64 | 60 (local, SHM), 30 (SSH, compressed) | yes | Richest control surface; the primary target |
| **Sixel** | Windows Terminal ≥1.22, foot, WezTerm*, Konsole, iTerm2, mintty, Rio, Contour, mlterm, tmux (built with Sixel) | Palette-indexed, full raster per frame | ~0.3–0.9 MB after RLE | 30 | via palette | Universal-ish, but encode cost is CPU-bound |
| **iTerm2 inline images** | iTerm2, WezTerm* | Full base64 PNG/JPEG per frame | 0.2–1.0 MB (JPEG/PNG) | 15–30 | via PNG | Simple but no in-place update; needs an encoder in-process |
| **ANSI truecolor half-blocks** | Anything with 24-bit color | Text repaint, 2 vertical pixels per cell | ~150–250 KB | 30–60 (small viewports) | no | Always works; the `▀` foreground/background trick |
| **ASCII** | Anything | Text repaint, 1 glyph per cell | ~7–15 KB | 60+ | no | The parrot.live aesthetic; `--backend ascii` |

\* WezTerm requires `enable_kitty_graphics = true` for Kitty graphics; Sixel and
iTerm2 are also available, which makes it the safest choice on Windows.

Alacritty has no graphics protocol at all (Sixel was rejected upstream, Kitty
graphics unimplemented): it is an ANSI/ASCII-only target.

## 2. Kitty graphics protocol

Escape form: `ESC _ G <k=v,...> ; <base64 payload> ESC \`, i.e. an APC string.

Facts that drive the design:

- **Pixel formats:** `f=24` RGB, `f=32` RGBA (default), `f=100` PNG.
- **Compression:** `o=z` (zlib) applies to the payload before base64.
- **Transport:** `t=d` direct (chunked, ≤ 4096 bytes of base64 per escape, `m=1`
  except the last), `t=f` a file the terminal reads, `t=t` a temp file the terminal
  deletes, `t=s` shared memory (POSIX shm, or a named shared memory object on
  Windows). The non-direct transports **skip base64 entirely**, which is the single
  biggest local-performance lever: the payload is just a path.
- **Placement:** transmit once with an id `i=…` (or image number `I=…` to let the
  terminal allocate the id), then display with `a=p`. Placement id `p=…` lets you
  replace/resize a placement without flicker. `C=1` stops the cursor from moving.
- **Sizing:** `c=`/`r=` request a cell rectangle; the terminal scales while preserving
  aspect ratio. Use `c`/`r` for the final fit so *we* can render at a smaller raster
  than the cell rectangle and let the terminal upscale (huge bandwidth win).
- **Animation:** `a=f` transmits frames (with `x,y,s,v` partial rectangles, `c=` base
  frame, `z=` inter-frame gap in ms, gapless `z<0`), `a=a` controls playback
  (`s=1` stop, `s=2` loading/streaming, `s=3` loop, `v=` loop count, `c=` current
  frame), `a=c` composes rectangles between frames. This enables **terminal-driven
  playback** and **delta frames** — transmit only tiles that changed.
- **Quotas:** terminals enforce a storage quota (kitty: 320 MB per buffer, animation
  frames on disk with a larger quota). Every transmit can carry the transient hint
  `N=1` so frames are evicted first.
- **Detection:** send `a=q` with a 1×1 dummy image followed by `ESC [ c` (primary DA).
  A graphics response without a DA response means no support. `q=1/2` silence
  responses.

Strategy: `prepare` allocates the image id and geometry once; each frame is one
`a=T` (or `a=f` in animated mode) with `o=z` and either direct chunks (remote) or
shared memory (local). On exit, `a=d,d=A` removes placements.

### Why not always use the animation path?

Terminal-driven animation needs frames transmitted ahead of time, which conflicts
with unbounded live sources and costs memory/disk quota. Delta frames help exactly
when consecutive frames are similar (screen content, animation), and hurt on noisy
live video. So: animated/delta mode is opt-in (`--mode animated`), chosen by source
class and measured, not a default.

## 3. Sixel

Sixel is a palette-indexed bitmap format: a header (`ESC P q` with raster attributes),
then six-row bands of characters encoding 6 vertical pixels each, with RLE `!n`.

- Requires quantization to a palette per frame; dithering improves gradients at a CPU
  cost. Plan: 6×6×6 color cube + grayscale ramp (240 colors) as the fast path, adaptive
  palette with median cut as `--dither adaptive` (M6).
- No in-place updates: every frame is a full repaint, so cap the raster (default
  budget: 960 px wide, 30 fps).
- Windows Terminal support starts at 1.22; tmux needs `--enable-sixel` at build time.

## 4. iTerm2 inline images

`ESC ] 1337 ; File=inline=1;width=…;height=…:<base64>BEL`. Simple, but each frame is a
full image (PNG/JPEG) upload with no reuse, no compression negotiation beyond the image
codec, and no partial updates. Suitable as a compatibility backend on macOS, not a
performance target.

## 5. ANSI truecolor half-blocks

Upper half block `▀` with foreground = top pixel and background = bottom pixel gives
2 vertical pixels per cell at 24-bit color. Cost is dominated by SGR sequences
(~20 bytes/cell), so use it at small viewports and low fps; batch rows, avoid
per-cell cursor moves, and only emit SGR when a color changes.

ASCII mode reuses the same renderer with a luminance ramp and is the `curl`-friendly
output of `serve` (parrot.live-style).

## 6. Capability detection order

1. Explicit `--backend <name>` (always wins; fails loudly if unsupported).
2. `FRAMEFEED_BACKEND` environment variable.
3. Terminal identity hints (`TERM`, `TERM_PROGRAM`, `KITTY_WINDOW_ID`,
   `WEZTERM_EXECUTABLE`, `TERM_PROGRAM_VERSION`) — cheap, optimistic.
4. Active probes, in order: Kitty graphics query (DA1 ordering), Sixel DA
   (`ESC [ ? 4 ; … c` with attribute 4), cell pixel size (`CSI 16 t`, `CSI 14 t`).
5. Fallback: ANSI half-blocks; if color depth < 24-bit, ASCII.

Probes must be time-bounded (a few hundred ms) and must never leave the terminal in
raw mode or a half-written state on timeout.

## 7. Multiplexer and SSH rules

- tmux: Kitty needs `allow-passthrough on` and each escape wrapped in
  `ESC P tmux;` … `ESC \` with inner `ESC` doubled. Sixel needs a Sixel-enabled tmux.
  Detect `$TMUX` and degrade instead of producing garbage.
- SSH: prefer compressed direct transmission, lower the raster budget, and surface a
  one-line hint (`--quality 360p`) instead of silently looking bad.

## 8. References

- Kitty graphics protocol: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>
- Sixel support matrix: <https://www.arewesixelyet.com/>
- Windows Terminal Sixel: <https://devblogs.microsoft.com/commandline/windows-terminal-preview-1-22-release/>
- mpv `--vo=kitty`: <https://mpv.io/manual/stable/>
