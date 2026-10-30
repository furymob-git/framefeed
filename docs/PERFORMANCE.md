# Performance

Performance is a feature with a budget, not a vibe. This document defines the targets,
the measurement method, and the regression policy. Targets are marked as targets;
section 7 records the first measured run.

## 1. Targets

| Scenario | Viewport | Raster | Target | Backend |
|---|---|---|---|---|
| Local file, best case | 160×45 cells (8×16 px) | 1280×720 | **60 fps**, < 60% of one core, 0 dropped frames | kitty + shared memory |
| Local file, default | 120×34 cells | 960×544 | **60 fps**, < 50% of one core | kitty + shared memory |
| Local file, compressed | 120×34 cells | 960×544 | **30 fps**, < 60% of one core | kitty + zlib + direct |
| Sixel | 100×30 cells | 800×480 | **30 fps**, < 80% of one core | sixel |
| ANSI half-blocks | 80×24 cells | 640×24 px | **30 fps**, < 40% of one core | ansi |
| SSH (20 Mbit/s) | 100×30 cells | 480p | **30 fps**, no audio drift | kitty + zlib |
| Startup | — | — | **< 150 ms** to first frame (local file) | any |
| Seek latency | — | — | **< 250 ms** to first frame after seek | any |

"One core" = one hardware thread on the reference machine; record the machine in the
benchmark output instead of hard-coding a CPU model.

## 2. Byte budgets

Approximate per-frame cost for a 1280×720 raster (see `PROTOCOLS.md` for the protocol
mechanics). Base64 inflates any direct payload by 33%, which is why shared memory and
zlib matter.

| Path | Bytes/frame | 60 fps | 30 fps |
|---|---|---|---|
| kitty, RGBA direct | 3.7 MB | 221 MB/s | 110 MB/s |
| kitty, RGBA + base64 | 4.9 MB | 294 MB/s | 147 MB/s |
| kitty, zlib + base64 (typical) | 1.0–2.0 MB | 60–120 MB/s | 30–60 MB/s |
| kitty, shared memory | 3.7 MB but no encode copy | 221 MB/s | 110 MB/s |
| sixel 240-color + RLE | 0.3–0.9 MB | 18–54 MB/s | 9–27 MB/s |
| ANSI half-blocks (160×45 cells) | ~200 KB | 12 MB/s | 6 MB/s |
| ASCII (160×45 cells) | ~10 KB | 0.6 MB/s | 0.3 MB/s |

Consequences baked into the design:

- The default raster is **not** the full cell-pixel area on every backend; kitty gets a
  `--raster-height` cap and lets the terminal upscale the placement.
- Base64 is never on the local kitty path.
- ANSI mode defaults to a smaller viewport and lower fps.
- `--stats` reports bytes/frame so budgets are observable, not aspirational.

## 3. Frame budget breakdown (targets)

For a 16.7 ms frame (60 fps):

| Stage | Budget | Notes |
|---|---|---|
| Decode | 6.0 ms | ffmpeg child process does the work in parallel; our cost is the pipe copy |
| Scale/fit | 1.5 ms | SIMD resizer, one pass, no intermediate RGBA→RGB conversion |
| Quantize/dither (if any) | 2.0 ms | sixel only |
| Encode | 5.0 ms | zlib level 1, reusable buffers, chunked writes |
| Write | 2.0 ms | one `write_all` per frame, no per-cell syscalls |
| Slack | 0.2 ms | — |

These are ceilings, not a plan to spend them. The measured baseline after M1 replaces
this table.

## 4. Measurement

- **Micro:** `criterion` benches for the resizer, each encoder, palette generation, and
  the pacer (pure logic). Run with `cargo bench`.
- **End to end:** `hyperfine` against generated fixtures
  (`ffmpeg -f lavfi -i testsrc2=…`) for startup, seek, and a fixed 10 s playback run.
  `--stats` output is machine-readable; `--stats-format json` feeds a small report
  script.
- **CPU profiles:** `perf record`/`flamegraph` on Linux, ETW + WPA on Windows, Instruments
  on macOS. Profiles are attached to performance PRs, not committed.
- **Regression policy:** benchmarks run on `main` only (noisy shared CI runners are not
  a merge gate). A PR that claims a speedup must include before/after numbers from the
  same machine and command. A >10% regression on a guarded benchmark opens a tracking
  issue.
- **Never claim a speedup without a measurement.** Comments and PR descriptions say
  "measured on X, command Y".

## 5. Optimization order

1. Remove work: smaller raster, fewer frames, no base64, no copies.
2. Remove allocations: reuse buffers, `Vec::clear`, arena for chunk metadata.
3. Remove syscalls: one write per frame, batched SGR in ANSI mode.
4. Cheaper math: integer/simd downscale, zlib level 1, `unsafe`-free SIMD via
   `fast_image_resize`.
5. Parallelism only when a stage is measured as the bottleneck (decode is already
   out-of-process).

## 6. Known ceilings

- Terminal emulator parsing and rasterization dominate at high frame rates; that is why
  the kitty placement lets the terminal upscale instead of us sending native pixels.
- PTY throughput is high locally but pays for every base64 byte twice (encode + parse).
- SSH bandwidth is the wall; the only real lever is fewer bytes per frame.
- `zlib` on noisy footage compresses poorly (~1.2×); it shines on screen content and
  animation. Compression must be adaptive, not religious.

## 7. First measurements (M1)

Environment: Windows 11 x86_64, Rust 1.98 stable (release profile), ffmpeg N-128212
(BtbN build), output piped to a file. These numbers therefore cover decode, scale,
encode and write — **not** the terminal's own rasterisation or PTY backpressure.

Sources: an 8 s excerpt of a 480×360 / 30 fps clip, a 1 s `testsrc2` synthetic clip,
and the public Mux HLS test stream (Big Buck Bunny) over HTTPS.

| Scenario | Raster | Frames | Dropped | Wall | Effective fps | Bytes/frame |
|---|---|---|---|---|---|---|
| ANSI, `--quality 240p --fps 24` | 320×240 | 240 | 0 | 10.02 s | 24.0 | 188 KiB |
| Kitty, `--quality 360p --fps 30` | 480×360 | 240 | 0 | 8.03 s | 29.9 | 29 KiB (zlib) |
| ANSI, synthetic 10 fps cap | 320×240 | 10 | 0 | 0.95 s | 10.5 | 378 KiB |
| HLS (Mux), 8 s cap, 10 fps | 427×240 | 67 | — | 8 s | — | ~290 KiB |

What this already tells us:

- Pacing is accurate to within 0.5% at both 24 and 30 fps, with zero drops on a
  local file.
- Kitty + zlib is **6.5× cheaper per frame** than ANSI at a comparable size
  (29 KiB vs 188 KiB), which is the whole argument for the Kitty backend.
- The synthetic test pattern is a worst case for ANSI (~378 KiB/frame): noise does
  not compress and produces maximal SGR churn.

> A first run reported 47 fps against a 24 fps cap. That measurement exposed a pacer
> bug: the wait path did not reserve the slot for the frame it was delaying, so every
> other frame presented immediately and playback ran at roughly double speed. Fixed,
> covered by a regression test, and re-measured above.

Still to measure: the 60 fps target at 120×34 cells with a real terminal attached,
Sixel budgets, startup latency and memory. Those are the M1 exit criteria the
benchmark suite (criterion + hyperfine) will track.
