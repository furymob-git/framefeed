# Streaming, sources and quality selection

Two related problems share the word "streaming" here:

1. **Playback** of a remote source (HLS/DASH/HTTP) — the player pulls, buffers, and
   paces frames.
2. **Serving** terminal video over HTTP so `curl` can watch it (`framefeed serve`) —
   the parrot.live model, but with real pixels and a quality knob.

## 1. Sources

| Source | Detection | Notes |
|---|---|---|
| Local file | path exists | ffmpeg handles container/codec detection |
| HTTP progressive (`mp4`, `webm`) | `http(s)://` + `Content-Type`/extension | seek via range requests when possible |
| HLS (`.m3u8`) | playlist parse | variant selection, live vs VOD |
| DASH (`.mpd`) | manifest parse (M3+, behind `--features dash`) | lower priority than HLS |
| `yt-dlp` | explicit `--via yt-dlp` | resolve to a direct URL, then play normally |

The source layer's job is to hand the decoder a URL or a byte stream plus metadata
(duration, live flag, variants). It does not decode.

## 2. Quality model

`--quality` accepts `144p | 240p | 360p | 480p | 720p | 1080p | auto` and maps to a
**target raster height in pixels**: 144, 240, 360, 480, 720, 1080 (auto = terminal
geometry, capped by backend budget).

Two independent effects:

1. **Render budget** — the raster we build never exceeds the target height (and the
   backend budget). Fewer pixels means less scale, encode and write work.
2. **Source selection** — for adaptive sources, choose the *smallest* variant whose
   height is ≥ the target. If none is large enough, choose the largest. If the
   terminal is tiny, we do not download a 1080p stream to throw 90% of it away.

Selection algorithm (HLS):

```text
target = quality_to_height(q) or auto_height(terminal, backend)
variants = parse_master_playlist()            # RESOLUTION, BANDWIDTH, CODECS, URL
eligible = variants.filter(|v| v.height >= target)
choose = eligible.min_by(height, then bandwidth)
         or variants.max_by(height) if eligible is empty
```

**Status (M1):** `select_variant` is implemented and exercised by `framefeed probe`.
The `play` path currently passes the URL straight to ffmpeg, which applies its own
variant heuristic, so `--quality` reduces pixels but not bytes downloaded yet. Hooking
our selection into the source layer before spawning the decoder is the M3 task that
makes the bandwidth promise real.

Additional rules:

- Prefer variants whose codecs the decoder can handle without a software fallback.
- `--max-bitrate` caps the chosen variant's `BANDWIDTH`.
- Live streams: switch variants only at segment boundaries, never mid-segment.
- `auto` re-evaluates on terminal resize, with hysteresis to avoid flapping.
- Report the decision: `--probe` and `--stats` include the chosen variant and why.

## 3. Buffering and resilience

- Decoder reads ahead into a small ring buffer (target: 2–3 s for VOD, 1–2 s for live).
- Stalls are visible: `--stats` counts them; the status line shows `buffering`.
- Retries: exponential backoff with jitter, capped (3 attempts, 5 s max), then a clear
  error naming the failing URL/variant.
- Live edge policy: stay 2 segments behind the live edge; if we fall further behind,
  skip forward rather than accumulate delay.
- HTTP: timeouts (connect 5 s, read 10 s), redirect cap (5), no proxy surprises;
  `Range` requests for VOD seeking.

## 4. `serve`: curl mode

```console
$ framefeed serve --listen 127.0.0.1:8080 --max-streams 4
$ curl -N 'http://127.0.0.1:8080/stream?url=./clip.mp4&q=144p&backend=ansi'
```

Response: an endless `text/plain` escape
stream. Each frame is `CSI H` (cursor home) + encoded frame; the client terminal paints
at whatever rate bytes arrive. No HTML, no MJPEG, no JavaScript — it must work with
plain `curl` piping to a tty.

Query parameters:

| Param | Values | Default |
|---|---|---|
| `url` | path or http(s) URL | required |
| `q` | `144p…1080p`, `auto` | `auto` |
| `backend` | `ansi`, `kitty`, `sixel` | `ansi` |
| `fps` | 1–60 | backend-dependent |
| `cols`, `rows` | integers | client-provided via query, else 80×24 |

Constraints (non-negotiable, they are the abuse surface):

- **SSRF guard (implemented):** scheme allowlist (`http`, `https`, and local paths
  only when `--allow-local` is set); DNS resolution rejecting loopback, private,
  link-local, CGNAT, ULA and multicast addresses; `file://` and other schemes are
  refused before any process is spawned.
- **Limits (partial):** max concurrent streams enforced (429 beyond it) plus request
  read/write timeouts. Per-IP rate limiting, max duration and max bytes per stream
  are still to do.
- **Residual risk:** a *public* playlist can reference private segment URLs that
  ffmpeg would still fetch. Closing that needs demuxer-level allowlisting.
- **No shell:** never interpolate user input into a command line; exec with argument
  vectors and an explicit ffmpeg protocol whitelist.
- **Resource caps:** one decoder process per stream, killed on disconnect; no shared
  mutable state between streams.
- **Observability:** `/healthz`, structured logs with request id and byte counts.

## 5. Why not just pipe mpv or ffmpeg output?

Because the interesting work is the terminal encoder: mpv can target kitty but not our
quality policy, sixel budgets, or `curl`-friendly ANSI frame pacing, and ffmpeg cannot
speak any graphics protocol. The decoder is a commodity; the encoder and the pacing are
the product.
