#!/usr/bin/env bash
# Watch a video that is served by the local framefeed server, as a curl client.
# Requires ./scripts/serve.sh to be running in another terminal.
#   ./scripts/play-server.sh                              # fixture hosted by the server
#   ./scripts/play-server.sh path/to/video.mp4
#   ./scripts/play-server.sh https://host/master.m3u8     # public URL relayed by the server
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_common.sh"

require_curl

SOURCE="${1:-}"
if [ -z "$SOURCE" ]; then
    SOURCE="$(ls "$ROOT"/videos/*.mp4 "$ROOT"/videos/*.webm "$ROOT"/videos/*.mkv 2>/dev/null | head -1 || true)"
fi
if [ -z "$SOURCE" ]; then
    SOURCE="target/fixtures/bad-apple-8s.mp4"
fi

if ! curl -fsS --max-time 2 "http://127.0.0.1:${PORT}/healthz" >/dev/null 2>&1; then
    echo "no server on 127.0.0.1:${PORT} - start it with ./scripts/serve.sh" >&2
    exit 1
fi

# The server cannot detect this terminal: resolve "auto" here.
if [ "$BACKEND" = "auto" ]; then
    if [ -n "${WT_SESSION:-}" ]; then
        BACKEND=sixel
    else
        BACKEND=ansi
    fi
fi

# --data-urlencode + -G build the query string, so paths and URLs are encoded.
args=(-sS -N -G "http://127.0.0.1:${PORT}/stream"
    --data-urlencode "url=${SOURCE}"
    --data-urlencode "q=${QUALITY}"
    --data-urlencode "backend=${BACKEND}"
    --data-urlencode "cols=${COLS}"
    --data-urlencode "rows=${ROWS}")
if [ -n "$FPS" ]; then
    args+=(--data-urlencode "fps=${FPS}")
fi
exec curl "${args[@]}"
