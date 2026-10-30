# Deploying `framefeed serve`

`framefeed serve` is a stateless HTTP service: it takes a source URL, decodes it
with the bundled ffmpeg, and streams terminal escape sequences to the client. The
image ships ffmpeg, so the host needs nothing else.

## What gets deployed

| Piece | Where |
|---|---|
| `Dockerfile` | multi-stage: Rust build, then `debian:bookworm-slim` + ffmpeg |
| `.github/workflows/deploy.yml` | builds and pushes `ghcr.io/<owner>/<repo>` on `v*` tags or manual dispatch |
| `docker-compose.yml` | local run on `127.0.0.1:8080` |

The container entrypoint is `framefeed serve --listen 0.0.0.0:8080`, **without**
`--allow-local`: only `http(s)` sources are served, never files from the host.

## Publish the image

The workflow is the "git deploy" step: pushing a tag publishes the image.

```console
$ git tag v0.1.0
$ git push origin v0.1.0
```

Result: `ghcr.io/<owner>/<repo>:v0.1.0`, `:sha-<commit>` and `:latest` (default
branch tags only). The workflow needs no secrets beyond `GITHUB_TOKEN`; package
visibility is set in the repository's package settings (private by default).

Manual run: *Actions → Container image → Run workflow*.

## Run it

```console
$ docker build -t framefeed .
$ docker run --rm -p 8080:8080 framefeed
$ curl 'http://127.0.0.1:8080/healthz'
ok
$ curl -N 'http://127.0.0.1:8080/stream?url=https%3A%2F%2Fexample.com%2Fclip.mp4&q=360p'
```

Or `docker compose up --build`.

| Query | Values | Default |
|---|---|---|
| `url` | required, `http(s)` only in the image | — |
| `q` | `144p…1080p`, `auto` | `auto` |
| `backend` | `ansi`, `kitty`, `sixel` | `ansi` |
| `fps` | 1–60 | backend dependent |
| `cols`, `rows` | integers | 80×24 |
| `fit` | `contain`, `cover`, `stretch` | `contain` |

## Hosting

Any container host works. Two examples:

```console
# Fly.io
$ fly launch --image ghcr.io/<owner>/<repo>:latest --internal-port 8080

# a VPS with Docker
$ docker run -d --restart unless-stopped -p 8080:8080 ghcr.io/<owner>/<repo>:latest
```

Put it behind TLS (reverse proxy or the platform) before exposing it publicly.

## Security and limits

- **SSRF guard**: scheme allowlist, then DNS resolution rejecting loopback,
  private, link-local, CGNAT and ULA addresses. A public deployment cannot be
  used to probe your internal network.
- **No local files**: `--allow-local` is not in the image. If you need it, build
  your own image and mount the media — never expose that publicly.
- **Concurrency**: `--max-streams 4` by default; extra clients get `429`. Raise it
  only with the bandwidth to match: one 1080p Sixel stream is ~100 KiB/frame.
- **No authentication**: add it at the proxy (basic auth, token, allowlist). The
  service is CPU- and bandwidth-hungry by nature, so rate limit per IP.
- **Health check**: `GET /healthz` returns `ok`; use it for liveness probes.
- **Timeouts**: read/write timeouts and a bounded stream count are already in
  the server; a stalled client cannot pin a decoder forever.
