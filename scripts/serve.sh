#!/usr/bin/env bash
# Start the local streaming server (curl-friendly /stream endpoint).
#   ./scripts/serve.sh
#   PORT=9000 ./scripts/serve.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_common.sh"

ensure_binary
ensure_ffmpeg

echo "framefeed serve on http://127.0.0.1:${PORT}  (Ctrl-C to stop)"
echo "  play through it: ./scripts/play-server.sh target/fixtures/bad-apple-8s.mp4"
echo "  curl stream:     curl -N 'http://127.0.0.1:${PORT}/stream?url=target/fixtures/bad-apple-8s.mp4&q=240p'"
echo "  health:          curl 'http://127.0.0.1:${PORT}/healthz'"
echo "  clips by name:   curl -N 'http://127.0.0.1:${PORT}/'   (demo library)"
echo "  one clip:        curl -N 'http://127.0.0.1:${PORT}/sintel?backend=sixel'"
echo "  your own media:  curl -N 'http://127.0.0.1:${PORT}/stream?url=videos/mon-film.mp4'"

exec "$BIN" serve --allow-local --listen "127.0.0.1:${PORT}" --media "$ROOT/videos/demo"
