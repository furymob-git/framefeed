# Architecture

Status: design (M0). This document is the contract the implementation is written
against; changes to the pipeline or threading model require an ADR (see `adr/`).

## 1. Goals and non-goals

**Goals**

- Stable frame pacing for local playback: no cumulative drift, no growing latency.
- Degrade across display protocols without changing the pipeline above the encoder.
- Choose the cheapest adequate source variant for adaptive streams.
- Predictable resource use: one core, bounded memory, no unbounded queues.
- Portable: Linux, macOS, Windows (Windows is the primary development target).

**Non-goals (1.0)**

- A general media framework: no filtering graph, no transcode/export.
- GUI, windows, GPU rendering of our own. We draw *into* the terminal.
- Embedding in editors/multiplexers as a library plugin (we expose a CLI + `serve`).
- Perfect A/V sync for exotic containers; correctness for common mp4/webm/HLS first.

## 2. Dataflow

```text
            ┌──────────────┐   Frame (RGBA, PTS)   ┌───────────────┐
 Source ───►│   Decoder    │───── capacity 3 ─────►│   Renderer    │──► Encoder ──► TtyWriter
 (file,     │ (sidecar or  │      drop-oldest      │ scale + fit + │    (protocol    (single
  http,     │  libav)      │                       │ quantize      │     bytes)      write)
  hls)      └──────────────┘                       └───────────────┘
                                     ▲                      │
                                     └──── Pacer clock ◄─────┘
```

Stages:

1. **Source** — resolves a user string to a byte stream plus metadata: local path,
   HTTP(S) progressive, HLS/DASH playlist. Owns retries, redirects, timeouts,
   variant selection (`--quality`), and (M3) live-stream reconnection.
2. **Decoder** — turns bytes into `Frame { width, height, stride, pixel_format, pts,
   duration }`. Two backends behind one trait: `ffmpeg` (child process, default) and
   `libav` (in-process bindings, feature-gated, M6).
3. **Renderer** — computes the output raster for the current terminal geometry
   (`cols × cell_w`, `rows × cell_h`), scales with a SIMD resizer, applies fit mode
   (`contain` / `cover` / `stretch`) and optional dithering/quantization for
   palette-based backends.
4. **Encoder** — pure function `(&Raster, &mut Vec<u8>) -> &[u8]`, one per display
   protocol. No I/O, no clock, no globals; golden tests assert exact bytes.
5. **TtyWriter** — owns the terminal: raw mode, alternate screen, cursor hiding,
   synchronized output, resize events, panic-safe restore. One `write_all` per frame
   (plus flushes), never a syscall per cell.

## 3. Threading and backpressure

- **Decoder thread** (or ffmpeg child process + reader thread).
- **Encoder/renderer thread** — all CPU-heavy per-frame work; keeps the writer simple.
- **Writer thread** — blocking writes to the tty; the only place that can stall on a
  slow terminal or SSH link.
- Channels are `crossbeam-channel`-style bounded queues with **drop-oldest** semantics
  and a frame-generation counter. If the writer falls behind, old frames are discarded
  and the picture stays live instead of drifting into slow motion.
- Never hold a lock across I/O. Never allocate in the steady state: buffers are owned
  per stage and reused (`Vec::clear` + `resize`, not `vec![]`).

Why not `async` in the hot path: the workload is CPU-bound, hand-off is 1–2 frames,
and blocking pipes to a child process are simpler to reason about than an executor.
`async` is confined to network I/O in `framefeed-source` if it earns its keep.

## 4. Timing

- Video-only playback uses a monotonic clock started at the first presented frame:
  `deadline(n) = t0 + pts(n) - start_pts`.
- A/V playback (M4) uses the **audio clock as master**: the pacer follows the audio
  device position, falls back to wall clock if audio is unavailable.
- Late frames: present immediately, do not sleep, and count a drop if more than one
  frame interval late. Early frames: sleep the remaining interval.
- `--fps N` caps presentation rate *above* the pacer: the pacer may still drop frames;
  it never duplicates them to fill time.
- Seeking flushes every queue, resets the clock, and issues a decoder flush/seek.

## 5. Terminal geometry and fit

- Cell pixels come from `TIOCGWINSZ` (`ws_xpixel/ws_ypixel`) or `CSI 16 t`. Terminals
  that report 0 fall back to a conservative default cell aspect ratio and an explicit
  `--cell-size 8x16` override.
- Raster size = `cols × cell_w` by `rows × cell_h`, capped by `--raster-height`
  (backend budgets differ; see `PERFORMANCE.md`).
- Fit modes keep the source aspect ratio: letterbox (`contain`, default), crop
  (`cover`), or distort (`stretch`). Sub-cell rounding is done once, not per frame.
- Resize handling: `SIGWINCH` on Unix, resize events via the terminal backend on
  Windows; debounce ~50 ms, recompute geometry, reallocate buffers, redraw.

## 6. Display backend contract

```rust
// Illustrative; final shape lands with M1.
trait Backend {
    fn name(&self) -> &'static str;
    fn budget(&self) -> Budget;                 // max raster, max fps, bytes/frame
    fn prepare(&mut self, geom: Geometry, out: &mut Vec<u8>) -> Result<()>;
    fn encode(&mut self, frame: &Raster, out: &mut Vec<u8>) -> Result<()>;
    fn restore(&mut self, out: &mut Vec<u8>) -> Result<()>;
}
```

Rules:

- Encoders are byte-oriented and testable without a terminal.
- Every backend has a **capability probe** (`doctor`) and a **budget**; the pipeline
  never selects a backend without knowing its limits.
- Allocation of terminal resources (image ids, shared memory, temp files) happens in
  `prepare`, and release happens in `restore`, which also runs on error and panic.
- Kitty specifics: reuse one image id + placement (`a=p`), `C=1` to avoid cursor
  churn, `o=z` compression, shared-memory transport (`t=s`) locally, and the
  animation/partial-frame path as an opt-in mode (see `PROTOCOLS.md`).
- Sixel/iTerm2 cannot update in place: they repaint the whole raster each frame, so
  their budgets are stricter by construction.

## 7. Errors, cleanup, observability

- `thiserror` in libraries, `anyhow` at the CLI boundary; errors carry the stage and
  the source. No panics on user input; panics are for broken invariants only.
- A terminal guard restores the terminal (raw mode, cursor, screen, protocol state)
  on normal exit, on error, and on panic (`Drop` + panic hook).
- `--stats` emits periodic JSON lines (fps, frames decoded/presented/dropped, bytes
  per frame, stage latencies). `--log` uses `tracing` with `RUST_LOG` filtering.

## 8. Portability notes

- **Windows:** no `SIGWINCH`; terminal size events come from the console API. Use
  WezTerm (Kitty graphics) or Windows Terminal 1.22+ (Sixel). Shared-memory
  transport for Kitty is a named shared memory object: implement it behind a
  capability probe and a spike (M6), because emulator support is uneven.
- **Unix:** `TIOCGWINSZ`, `SIGWINCH`, `stty`-free raw mode via `crossterm`/`termios`.
- **tmux/screen:** Kitty graphics need passthrough (`allow-passthrough on`) and
  wrapping in a DCS passthrough envelope; Sixel needs a tmux build with Sixel. Both
  are detected, not assumed.
- **SSH:** bandwidth, not CPU, is the limit. Budgets switch to "compressed direct
  transmission" automatically and `--quality` should be honored strictly.

## 9. Planned crate split (M1)

| Crate | Responsibility | Depends on |
|---|---|---|
| `framefeed-core` | `Frame`, `Raster`, geometry, pacer, traits, pipeline | — |
| `framefeed-proto` | encoders, capability detection, terminal guard | core |
| `framefeed-decode` | sidecar + libav backends | core |
| `framefeed-source` | file/http/HLS, variant selection | core |
| `framefeed-cli` | argument parsing, wiring, `play`/`serve`/`doctor` | all |
| `framefeed-server` | `serve` HTTP endpoint (M5) | source, core |
