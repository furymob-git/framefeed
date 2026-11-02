# Deploying framefeed on Vercel

This deployment turns framefeed into a small, DB-driven streaming service:

- `GET /` — static catalog page listing published clips.
- `GET /api/videos` — JSON catalog used by the page.
- `GET /<slug>` — plays one clip as ANSI truecolor frames in a terminal.
- `GET /healthz` — liveness probe used by the local dev server.

There is no ffmpeg, no native binary and no video file at runtime. Clips are
converted **on your machine** into gzipped raw RGB565LE frames, stored in Vercel
Blob, and described by a row in Neon Postgres. The function downloads the `.gz`,
decompresses it in memory, converts each frame to ANSI half-blocks and paces the
HTTP response at the clip's frame rate.

```
browser ──► public/ (static catalog)
                 │ fetch
                 ▼
            /api/videos ──► Neon Postgres (metadata only)

curl ──► /<slug> ──► Vercel Function ──► Vercel Blob (frames.bin.gz)
                         │
                         └─ RGB565 → ANSI → chunked HTTP response
```

## What you need

- A GitHub account and a Vercel account.
- A Neon Postgres database and a **public** Vercel Blob store, both created from
  the Vercel dashboard (Storage → Create).
- Node.js 20.12 or newer locally.
- `ffmpeg` and `ffprobe` locally, for publishing. They are **not** deployed.

The repository stays public. Every credential lives in Vercel's environment
variables and in an untracked `.env.local`; nothing secret is committed.

## 1. Import the project

1. Push this repository to GitHub.
2. In Vercel: **Add New → Project → Import** the repository.
3. Framework preset: **Other**. Root directory: the repository root.
4. Leave the build command empty (there is no build step). `vercel.json`
   already sets `outputDirectory` to `public` and configures the functions.
5. Deploy once. The first deploy can succeed before the database and Blob store
   exist; the catalog will return `503` until step 2 is done.

CLI alternative:

```console
$ npm install -g vercel
$ vercel link          # link this directory to the Vercel project
$ vercel --prod
```

## 2. Create the backing stores

### Neon Postgres

In the Vercel dashboard: **Storage → Create → Neon**. Attach it to the project
and choose the region closest to your users. The integration adds `DATABASE_URL`
(and several `POSTGRES_*` aliases) to the project.

Only the metadata table lives in Postgres. Frames never touch the database.

### Vercel Blob

**Storage → Create → Blob**, attach it to the project, and choose **public**
access. The integration adds `BLOB_READ_WRITE_TOKEN`.

A public store is required because the streaming function fetches the blob over
HTTPS. Object keys include a random suffix, so URLs are unlisted rather than
secret. Do not publish media you are not allowed to redistribute.

Both integrations normally add their variables to Development, Preview and
Production. Verify under **Settings → Environment Variables** that
`DATABASE_URL` and `BLOB_READ_WRITE_TOKEN` exist for the environments you use.

## 3. Pull the environment locally

```console
$ vercel env pull .env.local
```

If the variables were added only to Production, pull that environment instead:

```console
$ vercel env pull .env.local --environment=production
```

`.env.local` is git-ignored. Never commit it. If you prefer to write it by hand,
copy `.env.example` and fill in the values.

Then install dependencies and create the table:

```console
$ npm ci
$ npm run db:init
```

`db:init` runs `scripts/schema.sql` statement by statement and is idempotent.

## 4. Publish a clip

Publishing is a local operation. It needs `ffmpeg`/`ffprobe`, the Blob token and
the database URL.

```console
$ npm run publish -- ./videos/rick-roll.private.mp4 \
    --slug rickroll \
    --title "Rick Roll" \
    --description "Never gonna give you up."
```

What happens:

1. `ffprobe` reads the source duration and size.
2. `ffmpeg` scales the clip to `cols × (rows * 2)` pixels, pads to preserve the
   aspect ratio, drops audio/subtitles/data, and emits raw `rgb565le` frames.
3. The frames are gzipped to `target/vercel/<slug>.bin.gz` and a metadata file
   is written next to it.
4. The `.gz` is uploaded to `videos/<slug>/frames.bin.gz` with a random suffix.
5. The `videos` row is inserted or updated (`published = true`).

Useful options:

| Option | Default | Notes |
|---|---|---|
| `--cols` / `--rows` | `80` / `24` | Text cells. Pixels are `cols × rows*2`. |
| `--fps` | `24` | Accepted range: 5–30. |
| `--start` | `0` | Skip seconds at the beginning. |
| `--duration` | full clip | Trim the clip, in seconds. |
| `--dry-run` | off | Extract only; no database or Blob writes. |
| `--ffmpeg` / `--ffprobe` | `ffmpeg` / `ffprobe` | Full paths, or set `FRAMEFEED_FFMPEG` / `FRAMEFEED_FFPROBE`. |

Always do a dry run first when trying a new source:

```console
$ npm run publish -- ./videos/clip.mp4 --slug clip --dry-run
$ npm run dev
$ curl -N http://127.0.0.1:3000/clip
```

The local dev server reads `target/vercel/*.json` and `*.bin.gz`; it needs no
Neon or Blob credentials. `http://127.0.0.1:3000/` serves the real catalog page
with the fixture metadata.

### Duration limits

`MAX_STREAM_SECONDS` defaults to `285` (a safety margin under Hobby's 300-second
function limit). A clip longer than that returns `422` with an explanation. The
publish script warns when the encoded clip exceeds 285 seconds.

- **Hobby:** maximum 300 s per invocation, including the streamed response.
- **Pro / Enterprise:** 800 s maximum, or 1800 s in the extended beta.
- Set `MAX_STREAM_SECONDS=780` for Pro and keep `maxDuration: "max"` in
  `vercel.json`. Values above 800 require function-level configuration and are
  outside the default setup.

Clips over 285 s (for example a 734-second movie) cannot play on Hobby. Trim them
or move to Pro.

## 5. Deploy and test

```console
$ vercel --prod
# or push to the connected branch and let Vercel build
```

Then, from any truecolor terminal:

```console
$ curl -N https://<project>.vercel.app/rickroll
```

The catalog is at `https://<project>.vercel.app/`; the JSON is at
`/api/videos`. `curl -N` disables client-side buffering so frames appear live.
`Ctrl+C` stops playback and leaves your shell usable — the stream does not switch
to the alternate screen or hide the cursor.

Test several concurrent streams (for example three terminal windows) to confirm
the deployment handles simultaneous functions.

### Response size

Vercel's 4.5 MB request/response limit applies to **buffered** responses.
Streaming functions are exempt, which is why this design writes chunked frames
continuously. If you ever see `413 FUNCTION_PAYLOAD_TOO_LARGE` on a long clip,
the response is being buffered by a proxy or the function was rewritten to build
the body in memory; both should be fixed rather than worked around.

## Bandwidth and storage

ANSI output is content-dependent: a busy 80×24 clip at 24 fps can produce a few
megabytes per second. A full-length (≈3.5 minute) clip can therefore transfer
roughly 0.5–0.75 GB per view. Budget accordingly:

| Plan | Included Fast Data Transfer | Rough full-length views |
|---|---|---|
| Hobby | 100 GB / month | ~130–200 |
| Pro | 1 TB / month | ~1,300–2,000 |

Blob storage is much smaller: the gzipped frames for a 3–4 minute clip are
typically 15–30 MB, so the 1 GB free Blob tier holds dozens of clips. Blob
egress to the function counts against Blob transfer limits; check current
pricing before publishing large catalogs.

Short clips, lower `--fps`, a smaller `--cols`/`--rows`, or a lower
`MAX_STREAM_SECONDS` all reduce bandwidth immediately.

## Security model

- The catalog API exposes titles, durations and geometry, never Blob URLs.
- The streaming function reads `stream_url` from the database. Treat database
  write access as publish access.
- Blob objects are public-by-URL with random suffixes. This is obscurity, not
  authorization. Anyone with the URL can download the frames.
- The repository has no tokens. `.env*` is ignored, `.vercelignore` excludes it
  from uploads, and `videos/` is never uploaded to Vercel.
- `BLOB_READ_WRITE_TOKEN` can write to the store. Rotate it in the Vercel
  dashboard if it leaks, then update local `.env.local` and redeploy.

## Operations

Republish (same slug): run the publish command again. The row is updated, the
new blob is uploaded and the previous blob is deleted.

Unpublish:

```console
$ npm run unpublish -- --slug rickroll
```

This deletes the Blob object and the database row. It is idempotent.

Inspect the catalog:

```console
$ curl -s https://<project>.vercel.app/api/videos | jq
$ curl -sI https://<project>.vercel.app/rickroll   # HEAD: metadata only
```

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| `/` shows the page but the catalog is empty | No published rows; run `npm run db:init` and publish a clip. |
| `/api/videos` returns `503` | `DATABASE_URL` missing in the running environment, or the Neon store is not attached. Re-run `vercel env pull` and redeploy. |
| `/<slug>` returns `404` | Slug is not published, was mistyped, or the rewrite is not active. Check `/api/videos`. |
| `/<slug>` returns `422` | Clip duration exceeds `MAX_STREAM_SECONDS`. Trim it or raise the limit on Pro. |
| `/<slug>` returns `502` | Blob fetch failed: token/store mismatch, blob deleted, or the store is private. Re-publish and check the store's access mode. |
| `504 FUNCTION_INVOCATION_TIMEOUT` | The clip outlasted the plan's function limit. Shorten it or upgrade. |
| `413 FUNCTION_PAYLOAD_TOO_LARGE` | A buffered response hit 4.5 MB. This design streams; check for a proxy or a code change that buffers. |
| `ffmpeg could not be executed` | Install ffmpeg or pass `--ffmpeg` / `--ffprobe` (or `FRAMEFEED_FFMPEG` / `FRAMEFEED_FFPROBE`). |
| `npm` is blocked in PowerShell | Use `npm.cmd` (execution policy blocks `npm.ps1`). |
| No color / garbage characters | The terminal needs 24-bit color (`COLORTERM=truecolor`). ANSI output is not browser video. |
| Stream stalls behind a proxy | Use `curl -N`; some proxies buffer `text/plain`, which is why the endpoint sends `application/octet-stream` with `no-transform`. |

## How the endpoint works

- Frame file: raw RGB565 little-endian, exactly `width_px * height_px * 2`
  bytes per frame, concatenated, then gzipped.
- Metadata row: geometry, fps, frame count, duration, size and Blob URL.
- The function fetches the whole `.gz` (tens of MB), inflates it as a stream,
  splits it into frames, converts each frame to truecolor ANSI half-blocks and
  writes at `1000 / fps` ms intervals. Backpressure is respected with `drain`.
- Headers: `Content-Type: application/octet-stream`,
  `Cache-Control: no-store, no-transform`, `X-Framefeed-Fps`,
  `X-Framefeed-Frames`, `X-Framefeed-Duration`. No `Content-Length`.
- The response starts with `ESC[2J ESC[H` (clear and home) and ends with
  `ESC[0m` and a newline. It never enters the alternate screen and never hides
  the cursor, so an interrupted stream cannot strand the terminal.
