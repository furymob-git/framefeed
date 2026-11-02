import zlib from 'node:zlib';
import { Readable } from 'node:stream';
import { encodeAnsiFrame } from './ansi.js';

// No alternate screen and no cursor hiding: if the viewer presses Ctrl+C the
// shell keeps a usable terminal. Rows reset their colors themselves, so a
// missing RESTORE after an abort cannot leak SGR state.
const PREAMBLE = '\x1b[2J\x1b[H';
const RESTORE = '\x1b[0m\n';
const SLUG_RE = /^[a-z0-9][a-z0-9-]{0,62}$/;
const MAX_SIXEL_FRAME = 4 * 1024 * 1024;

function sendPlain(res, status, text) {
  if (res.headersSent) {
    try {
      res.end();
    } catch {}
    return;
  }
  res.statusCode = status;
  res.setHeader('Content-Type', 'text/plain; charset=utf-8');
  res.end(text);
}

function sanitize(err) {
  if (!err) return 'unknown error';
  return err.message || String(err);
}

export function createStreamHandler({
  getVideo,
  maxSeconds = 285,
  fetchImpl = globalThis.fetch,
} = {}) {
  return async function handler(req, res) {
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.statusCode = 405;
      res.setHeader('Allow', 'GET, HEAD');
      res.end();
      return;
    }

    const url = new URL(req.url || '/', 'http://localhost');
    let slug = url.searchParams.get('slug');
    if (!slug) {
      const parts = url.pathname.split('/').filter(Boolean);
      slug = parts.length ? parts[parts.length - 1] : '';
    }

    if (!SLUG_RE.test(slug)) {
      sendPlain(res, 404, 'video not found\n');
      return;
    }

    let video;
    try {
      video = await getVideo(slug);
    } catch (err) {
      console.error('stream getVideo error:', sanitize(err));
      sendPlain(res, 503, 'catalog unavailable\n');
      return;
    }
    if (!video) {
      sendPlain(res, 404, 'video not found\n');
      return;
    }
    if (video.durationSeconds > maxSeconds) {
      sendPlain(
        res,
        422,
        `video duration ${video.durationSeconds}s exceeds stream limit ${maxSeconds}s; set MAX_STREAM_SECONDS or use a Pro plan\n`
      );
      return;
    }

    const isPosInt = (n) => Number.isFinite(n) && Number.isInteger(n) && n > 0;
    if (
      !isPosInt(video.widthPx) ||
      !isPosInt(video.heightPx) ||
      !isPosInt(video.fps) ||
      !isPosInt(video.frameCount)
    ) {
      sendPlain(res, 500, 'invalid video metadata\n');
      return;
    }
    const frameBytes = video.widthPx * video.heightPx * 2;
    if (!Number.isFinite(frameBytes) || frameBytes <= 0) {
      sendPlain(res, 500, 'invalid video metadata\n');
      return;
    }

    const format = video.format || 'ansi';

    const headers = {
      'Content-Type': 'application/octet-stream',
      'Cache-Control': 'no-store, no-transform',
      'X-Content-Type-Options': 'nosniff',
      'X-Framefeed-Fps': String(video.fps),
      'X-Framefeed-Frames': String(video.frameCount),
      'X-Framefeed-Duration': String(video.durationSeconds),
      'X-Framefeed-Format': format,
    };

    if (req.method === 'HEAD') {
      res.writeHead(200, headers);
      res.end();
      return;
    }

    let aborted = false;
    let gunzipRef = null;
    let inputRef = null;
    const abort = () => {
      aborted = true;
      if (gunzipRef) {
        try {
          gunzipRef.destroy();
        } catch {}
      }
      if (inputRef) {
        try {
          inputRef.destroy();
        } catch {}
      }
    };
    res.on('close', abort);

    let fetchRes;
    try {
      fetchRes = await fetchImpl(video.streamUrl);
    } catch (err) {
      console.error('stream fetch error:', sanitize(err));
      sendPlain(res, 502, 'media unavailable\n');
      return;
    }
    if (!fetchRes || !fetchRes.ok) {
      sendPlain(res, 502, 'media unavailable\n');
      return;
    }

    res.writeHead(200, headers);
    res.flushHeaders();

    gunzipRef = zlib.createGunzip();
    const gunzip = gunzipRef;
    let input;
    if (fetchRes.body) {
      try {
        input = Readable.fromWeb(fetchRes.body);
      } catch (err) {
        try {
          const ab = await fetchRes.arrayBuffer();
          input = Readable.from([Buffer.from(ab)]);
        } catch (e) {
          console.error('stream read error:', sanitize(e));
          sendPlain(res, 502, 'media unavailable\n');
          return;
        }
      }
    } else {
      try {
        const ab = await fetchRes.arrayBuffer();
        input = Readable.from([Buffer.from(ab)]);
      } catch (err) {
        console.error('stream read error:', sanitize(err));
        sendPlain(res, 502, 'media unavailable\n');
        return;
      }
    }
    inputRef = input;
    input.pipe(gunzip);

    const interval = 1000 / video.fps;

    async function* ansiFrames() {
      let buffer = Buffer.alloc(0);
      let count = 0;
      try {
        for await (const chunk of gunzip) {
          if (aborted) return;
          buffer = Buffer.concat([buffer, chunk]);
          while (buffer.length >= frameBytes && count < video.frameCount) {
            const frame = Buffer.from(buffer.subarray(0, frameBytes));
            buffer = buffer.subarray(frameBytes);
            count++;
            yield frame;
          }
          if (count >= video.frameCount) return;
        }
      } catch (err) {
        throw err;
      }
    }

    async function* sixelFrames() {
      let buffer = Buffer.alloc(0);
      let count = 0;
      try {
        for await (const chunk of gunzip) {
          if (aborted) return;
          buffer = Buffer.concat([buffer, chunk]);
          while (count < video.frameCount) {
            if (buffer.length < 4) break;
            const frameLength = buffer.readUInt32LE(0);
            if (frameLength <= 0 || frameLength > MAX_SIXEL_FRAME) {
              throw new Error('invalid sixel frame length');
            }
            if (buffer.length < 4 + frameLength) break;
            const frame = Buffer.from(buffer.subarray(4, 4 + frameLength));
            buffer = buffer.subarray(4 + frameLength);
            count++;
            yield frame;
          }
          if (count >= video.frameCount) return;
        }
      } catch (err) {
        throw err;
      }
      if (!aborted && count < video.frameCount) {
        throw new Error('unexpected end of sixel stream');
      }
    }

    let nextTime = performance.now();
    let i = 0;
    try {
      res.write(PREAMBLE);
      const frames = format === 'sixel' ? sixelFrames() : ansiFrames();
      for await (const frame of frames) {
        if (aborted) break;
        if (i > 0) {
          nextTime += interval;
          const now = performance.now();
          if (nextTime < now - interval) {
            nextTime = now;
          }
          const delay = nextTime - now;
          if (delay > 0) {
            await new Promise((resolve) => {
              let finished = false;
              const finish = () => {
                if (finished) return;
                finished = true;
                clearTimeout(t);
                res.off('close', finish);
                resolve();
              };
              const t = setTimeout(finish, delay);
              res.once('close', finish);
              if (aborted) finish();
            });
          }
          if (aborted) break;
        } else {
          nextTime = performance.now();
        }
        let encoded;
        if (format === 'sixel') {
          encoded = frame;
        } else {
          encoded = encodeAnsiFrame(frame, video.widthPx, video.heightPx);
        }
        const ok = res.write(encoded);
        if (!ok && !aborted) {
          await new Promise((resolve) => {
            const done = () => {
              res.off('drain', done);
              res.off('close', done);
              resolve();
            };
            res.once('drain', done);
            res.once('close', done);
          });
        }
        i++;
        if (i >= video.frameCount) break;
      }
    } catch (err) {
      console.error('stream decode error:', sanitize(err));
    } finally {
      try {
        gunzip.destroy();
      } catch {}
      try {
        input.destroy();
      } catch {}
    }

    if (!aborted) {
      try {
        res.write(RESTORE);
      } catch {}
    }
    try {
      res.end();
    } catch {}
  };
}