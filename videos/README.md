# videos/

Drop your own videos here. **Nothing in this folder is committed**: every file is
ignored except this README and `.gitignore`, so local media never ends up in git
history.

Supported containers are whatever the bundled `ffmpeg` can decode: `.mp4`,
`.webm`, `.mkv`, `.mov`, `.m4v`, `.avi`, …

## Play them

```bat
rem scripts automatically pick the first video in this folder
scripts\play.bat

rem or name one explicitly
scripts\play.bat videos\mon-film.mp4
```

```bash
./scripts/play.sh videos/mon-film.mp4
```

The first `*.mp4`, `*.webm` or `*.mkv` in alphabetical order is used when no path
is given. Headless test clips stay in `target/fixtures/` (also git-ignored), so
they do not mix with your own media.

## Served over HTTP

```bat
scripts\serve.bat
scripts\play-server.bat videos\mon-film.mp4
```

The server runs with `--allow-local` in the helper scripts, so it can read paths
inside this folder. Never enable that flag on a public deployment: there, pass
`url=` with an `http(s)` source instead (see [docs/DEPLOY.md](../docs/DEPLOY.md)).
