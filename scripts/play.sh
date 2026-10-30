#!/usr/bin/env bash
# Play a video directly in this terminal.
#   ./scripts/play.sh                       # 8 s Bad Apple fixture
#   ./scripts/play.sh path/to/video.mp4
#   ./scripts/play.sh https://host/master.m3u8
#   QUALITY=480p FPS=30 BACKEND=kitty ./scripts/play.sh video.mp4
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_common.sh"

ensure_binary
ensure_ffmpeg

if [ -z "${WT_SESSION:-}" ] && [ -z "${KITTY_WINDOW_ID:-}" ] && [ -z "${WEZTERM_EXECUTABLE:-}" ]; then
    echo "hint: no graphics protocol in this terminal; run this inside Windows Terminal 1.22+ for pixels" >&2
fi

# No argument: take the first video dropped in videos/, else the test fixture.
SOURCE="${1:-}"
if [ -z "$SOURCE" ]; then
    SOURCE="$(ls "$ROOT"/videos/*.mp4 "$ROOT"/videos/*.webm "$ROOT"/videos/*.mkv 2>/dev/null | head -1 || true)"
fi
if [ -z "$SOURCE" ]; then
    SOURCE="target/fixtures/bad-apple-8s.mp4"
fi

if [ ! -e "$SOURCE" ] && [[ "$SOURCE" != *"://"* ]]; then
    echo "no such file: $SOURCE" >&2
    echo "hint: fixtures live in target/fixtures (git-ignored); pass any path or URL" >&2
    exit 1
fi

args=(play "$SOURCE" --quality "$QUALITY" --backend "$BACKEND" --stats)
if [ -n "$FPS" ]; then
    args+=(--fps "$FPS")
fi
exec "$BIN" "${args[@]}"
