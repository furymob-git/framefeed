#!/usr/bin/env bash
# Shared setup for the local framefeed helper scripts. Sourced, never executed.
# This folder is git-ignored: it holds convenience wrappers with machine paths.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Override any of these from the environment, e.g. PORT=9000 QUALITY=480p ./scripts/serve.sh
PORT="${PORT:-8080}"
# auto = the binary measures the real terminal and sizes the raster itself.
QUALITY="${QUALITY:-auto}"
BACKEND="${BACKEND:-auto}"
FPS="${FPS:-}"
COLS="${COLS:-${COLUMNS:-$(tput cols 2>/dev/null || echo 80)}}"
ROWS="${ROWS:-${LINES:-$(tput lines 2>/dev/null || echo 24)}}"

if [ -x "$ROOT/target/release/framefeed.exe" ]; then
    BIN="$ROOT/target/release/framefeed.exe"
else
    BIN="$ROOT/target/release/framefeed"
fi

ensure_binary() {
    if [ -x "$BIN" ]; then
        return
    fi
    echo "framefeed binary missing, building with cargo build --release..." >&2
    cargo build --release
}

# Prefer the ffmpeg build unpacked under target/tools, then fall back to PATH.
ensure_ffmpeg() {
    if [ -n "${FRAMEFEED_FFMPEG:-}" ] && [ -x "${FRAMEFEED_FFMPEG}" ]; then
        return
    fi

    local candidate
    candidate="$(find "$ROOT/target/tools" -maxdepth 3 -type f \
        \( -name ffmpeg -o -name ffmpeg.exe \) 2>/dev/null | head -1 || true)"

    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
        export FRAMEFEED_FFMPEG="$candidate"
        case "$candidate" in
            *.exe) export FRAMEFEED_FFPROBE="$(dirname "$candidate")/ffprobe.exe" ;;
            *) export FRAMEFEED_FFPROBE="$(dirname "$candidate")/ffprobe" ;;
        esac
        return
    fi

    if command -v ffmpeg >/dev/null 2>&1 && command -v ffprobe >/dev/null 2>&1; then
        return
    fi

    echo "ffmpeg/ffprobe not found: install them on PATH, or unpack a build under target/tools" >&2
    exit 1
}

require_curl() {
    if ! command -v curl >/dev/null 2>&1; then
        echo "curl is required for this script" >&2
        exit 1
    fi
}
