# 0002 — Decoder: ffmpeg sidecar first, libav later

- Status: Accepted
- Date: 2026-10-30

## Context

We need demuxing and decoding for mp4/webm/mkv plus HLS/DASH, on three platforms. Two
realistic options:

1. **Sidecar process**: spawn `ffmpeg -i <src> -f rawvideo -pix_fmt rgba -` and read
   frames from stdout.
2. **In-process libav** via `ffmpeg-next`/`ffmpeg-sys-next` bindings.

Windows makes option 2 expensive: prebuilt FFmpeg dev libraries are not standard, MSVC
linking is fragile, and CI would need a vcpkg/gpac setup on every runner.

## Decision

Ship the sidecar backend first (M1). Define a `Decoder` trait so an in-process `libav`
backend can land in M6 behind a feature flag, without touching the renderer or
encoders.

## Consequences

- Zero build-time FFmpeg dependency for users: `framefeed` works if `ffmpeg` is on
  `PATH`, with a clear error and install instructions if it is not.
- All container/codec/HLS complexity is delegated to a battle-tested binary; our
  quality-selection logic stays in `framefeed-source`.
- Cost: one process per stream, a pipe copy per frame, and less control over precise
  seeking (we send `-ss` and flush).
- Cost: two moving parts in bug reports; `doctor` must print the ffmpeg version.
- Mitigation: raw RGBA frames with a fixed stride, a bounded read buffer, and a single
  reader thread. Measure the pipe copy before optimizing it.
- Security: ffmpeg parses untrusted media. Run it with an explicit protocol whitelist,
  no shell interpolation, and a resource cap (M5 server mode).

## Alternatives considered

- **GStreamer**: heavier, plugin discovery is a support burden, worse on Windows.
- **Pure-Rust decoders** (`openh264`, AV1 via `dav1d` bindings, VP9): incomplete
  coverage, inconsistent quality, and we would own codec bugs forever.
- **Link to libav always**: better latency and no pipe, but unacceptable install
  friction for a CLI people try in one command.
