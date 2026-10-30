# Bundled demo clips

Short, **silent** excerpts (14–18 s) from Blender Open Movies. All are licensed
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) /
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) and are redistributed
here, trimmed and re-encoded (H.264, ≤ 720 px tall, no audio, CRF 32), for
terminal-video demos.

| Name over HTTP | File | Film | Year | Licence | Source |
|---|---|---|---|---|---|
| `/big-buck-bunny` | `big-buck-bunny.mp4` | Big Buck Bunny | 2008 | CC BY 3.0 | <https://peach.blender.org/> |
| `/sintel` | `sintel.mp4` | Sintel | 2010 | CC BY 3.0 | <https://durian.blender.org/> |
| `/elephants-dream` | `elephants-dream.mp4` | Elephants Dream | 2006 | CC BY 2.5 | <https://orange.blender.org/> |
| `/caminandes` | `caminandes.mp4` | Caminandes 1: Llama Drama | 2013 | CC BY 3.0 | <https://caminandes.com/> |
| `/gran-dillama` | `gran-dillama.mp4` | Caminandes 2: Gran Dillama | 2013 | CC BY 3.0 | <https://caminandes.com/> |
| `/sprite-fright` | `sprite-fright.mp4` | Sprite Fright | 2021 | CC BY 4.0 | <https://studio.blender.org/films/sprite-fright/> |

© Blender Foundation — <https://www.blender.org/>. The films are the work of the
Blender Foundation and the respective studios; this repository claims no ownership
over them. Attribution is kept here, in `videos/README.md` and in the README.

## Why these and not the obvious memes

Bad Apple and the Rickroll are **not** in this repository: both are copyrighted
works (a Touhou fan animation and a Sony music video), so they cannot be
redistributed. Serve your own copy instead — nothing stops you:

```console
$ framefeed serve --allow-local --media videos
$ curl -N 'http://127.0.0.1:8080/bad-apple?q=360p'
```

with your file dropped in `videos/` (git-ignored, never committed).

## Regenerating

```bash
ffmpeg -t 14 -i <source> -an \
  -vf "scale=-2:'min(720,ih)':flags=lanczos" \
  -c:v libx264 -crf 32 -preset veryfast -pix_fmt yuv420p \
  -movflags +faststart videos/demo/<name>.mp4
```

Keep clips this short: the point is to demo terminal streaming, and each committed
megabyte is cloned by every contributor.
