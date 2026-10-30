# Performance

Performance is a feature with a budget, not a vibe. This document defines the targets,
the measurement method, and the regression policy. Until M1 benchmarks land, every
number here is a **target or an estimate**, explicitly marked as such.

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
