#!/usr/bin/env bash
# Play a video directly in this terminal.
#   ./scripts/play.sh                       # pick from videos/
#   ./scripts/play.sh path/to/video.mp4
#   ./scripts/play.sh https://host/master.m3u8
#   QUALITY=480p FPS=30 BACKEND=kitty ./scripts/play.sh video.mp4
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_common.sh"

ensure_binary
ensure_ffmpeg

if [ -z "${WT_SESSION:-}" ] && [ -z "${KITTY_WINDOW_ID:-}" ] && [ -z "${WEZTERM_EXECUTABLE:-}" ]; then
    echo "hint: no graphics protocol in this terminal; run this inside Windows Terminal 1.22+ for pixels" >&2
fi

# No argument: list the local videos and ask which one to play.
SOURCE="${1:-}"
if [ -z "$SOURCE" ]; then
    VIDEOS=()
    while IFS= read -r file; do
        VIDEOS+=("$file")
    done < <(find "$ROOT/videos" -type f \
        \( -iname '*.mp4' -o -iname '*.webm' -o -iname '*.mkv' \) | sort)

    if [ ${#VIDEOS[@]} -eq 0 ]; then
        echo "no videos found under $ROOT/videos" >&2
        echo "drop an .mp4/.webm/.mkv there, or pass a path or URL" >&2
        exit 1
    fi

    echo
    echo "Available videos:"
    echo
    for i in "${!VIDEOS[@]}"; do
        printf '  [%d] %s\n' "$((i + 1))" "${VIDEOS[$i]#"$ROOT/"}"
    done
    echo

    while :; do
        if ! read -r -p "Play which video? [1-${#VIDEOS[@]}, q to quit] " choice; then
            echo >&2
            exit 1
        fi
        case "$choice" in
            q | Q) exit 1 ;;
        esac
        if [[ "$choice" =~ ^[0-9]+$ ]]; then
            choice=$((10#$choice))
            if ((choice >= 1 && choice <= ${#VIDEOS[@]})); then
                SOURCE="${VIDEOS[$((choice - 1))]}"
                break
            fi
        fi
        echo "please enter a number between 1 and ${#VIDEOS[@]}, or q to quit" >&2
    done
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
