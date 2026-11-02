# videos/

Two things live here:

- **`demo/`** — a small, curated set of CC-BY Blender Open Movie excerpts that *are*
  committed (see [demo/CREDITS.md](demo/CREDITS.md)). They ship inside the Docker
  image and are the clips served by `GET /<name>`.
- **everything else** — your own media. **Never committed**: git ignores it, so your
  files stay out of history and out of the published image.

Supported containers are whatever the bundled `ffmpeg` can decode: `.mp4`, `.webm`,
`.mkv`, `.mov`, `.m4v`, `.ogv`, …

## Play them locally

```bat
rem scripts list the videos in videos/ and ask which one to play when no path is given
scripts\play.bat

rem or name one explicitly
scripts\play.bat videos\mon-film.mp4
```

```bash
./scripts/play.sh videos/mon-film.mp4
```

## Serve them over HTTP

```bat
rem serve the whole folder; then a clip is just /<file name>
scripts\serve.bat
curl -N "http://127.0.0.1:8080/mon-film"
```

The helper scripts pass `--allow-local` so `?url=<path>` works; `--media` (what
`/<name>` uses) never accepts a path, only a bare name resolved inside the folder.
Never enable `--allow-local` on a public deployment: there, pass `url=` with an
`http(s)` source (see [docs/DEPLOY.md](../docs/DEPLOY.md)).
