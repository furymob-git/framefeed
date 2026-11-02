import test from 'node:test';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import http from 'node:http';
import { createStreamHandler } from '../lib/stream.js';

function makeFrames(n = 3) {
  const w = 80, h = 48;
  const frameBytes = w * h * 2;
  const buf = Buffer.alloc(frameBytes * n);
  for (let f = 0; f < n; f++) {
    for (let i = 0; i < frameBytes; i += 2) {
      // gradient-ish RGB565
      const v = (f * 100 + (i / 2)) & 0xffff;
      buf[f * frameBytes + i] = v & 0xff;
      buf[f * frameBytes + i + 1] = (v >> 8) & 0xff;
    }
  }
  return { buf, w, h, n };
}

function gz(b) { return zlib.gzipSync(b); }

test('stream 200 body', async () => {
  const { buf, w, h, n } = makeFrames(3);
  const compressed = gz(buf);
  const getVideo = async (slug) => ({
    slug,
    title: 't',
    streamUrl: 'http://example/frames.bin.gz',
    widthPx: w,
    heightPx: h,
    fps: 60,
    frameCount: n,
    durationSeconds: 0.05,
  });
  const handler = createStreamHandler({ getVideo, maxSeconds: 285, fetchImpl: async () => new Response(compressed, { status: 200 }) });

  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/testslug`);
  assert.equal(res.status, 200);
  const body = Buffer.from(await res.arrayBuffer());
  const text = body.toString('utf8');
  assert.ok(text.startsWith('\x1b[2J\x1b[H'));
  assert.ok(!text.includes('\x1b[?1049h'), 'must not enter the alternate screen');
  assert.ok(!text.includes('\x1b[?25l'), 'must not hide the cursor');
  assert.ok(text.includes('▀'));
  const count = (text.match(/\x1b\[H/g) || []).length;
  assert.ok(count >= 3, `expected >=3 frame homes, got ${count}`);
  assert.ok(text.endsWith('\x1b[0m\n'));
  server.close();
});

test('stream 404 invalid slug', async () => {
  const handler = createStreamHandler({ getVideo: async () => null, maxSeconds: 285, fetchImpl: async () => new Response('', { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/BAD`);
  assert.equal(res.status, 404);
  const text = await res.text();
  assert.equal(text, 'video not found\n');
  server.close();
});

test('stream 404 null video', async () => {
  const handler = createStreamHandler({ getVideo: async () => null, maxSeconds: 285, fetchImpl: async () => new Response('', { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/validslug`);
  assert.equal(res.status, 404);
  server.close();
});

test('stream 422 duration too long', async () => {
  const getVideo = async (slug) => ({
    slug, title: 't', streamUrl: 'x', widthPx: 80, heightPx: 48, fps: 24, frameCount: 1, durationSeconds: 500,
  });
  const handler = createStreamHandler({ getVideo, maxSeconds: 10, fetchImpl: async () => new Response('', { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/validslug`);
  assert.equal(res.status, 422);
  server.close();
});

test('stream 405 method', async () => {
  const handler = createStreamHandler({ getVideo: async () => null, maxSeconds: 285, fetchImpl: async () => new Response('', { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/validslug`, { method: 'POST' });
  assert.equal(res.status, 405);
  server.close();
});

test('stream 3 frames at 10fps no premature abort', async () => {
  const { buf, w, h, n } = makeFrames(3);
  const compressed = gz(buf);
  const getVideo = async (slug) => ({
    slug, title: 't', streamUrl: 'http://example/frames.bin.gz',
    widthPx: w, heightPx: h, fps: 10, frameCount: n, durationSeconds: 0.3,
  });
  const handler = createStreamHandler({ getVideo, maxSeconds: 285, fetchImpl: async () => new Response(compressed, { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/testslug`);
  assert.equal(res.status, 200);
  const body = Buffer.from(await res.arrayBuffer());
  const text = body.toString('utf8');
  const count = (text.match(/\x1b\[H/g) || []).length;
  assert.ok(count >= 4, `expected preamble+3 frame homes, got ${count}`);
  assert.ok(text.endsWith('\x1b[0m\n'));
  server.close();
});

test('stream HEAD', async () => {
  const getVideo = async (slug) => ({
    slug, title: 't', streamUrl: 'x', widthPx: 80, heightPx: 48, fps: 24, frameCount: 1, durationSeconds: 1,
  });
  const handler = createStreamHandler({ getVideo, maxSeconds: 285, fetchImpl: async () => new Response('', { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/validslug`, { method: 'HEAD' });
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('x-framefeed-fps'), '24');
  server.close();
});

test('stream sixel 2 frames', async () => {
  const frame1 = Buffer.from('\x1b[H\x1bPq#0;2;0;0;0~\x1b\\');
  const frame2 = Buffer.from('\x1b[H\x1bPq#1;2;255;0;0~\x1b\\');
  function record(f) {
    const len = Buffer.alloc(4);
    len.writeUInt32LE(f.length, 0);
    return Buffer.concat([len, f]);
  }
  const compressed = zlib.gzipSync(Buffer.concat([record(frame1), record(frame2)]));
  const getVideo = async (slug) => ({
    slug,
    title: 't',
    streamUrl: 'http://example/frames.sixel.bin.gz',
    format: 'sixel',
    widthPx: 10,
    heightPx: 10,
    fps: 60,
    frameCount: 2,
    durationSeconds: 0.05,
  });
  const handler = createStreamHandler({ getVideo, maxSeconds: 285, fetchImpl: async () => new Response(compressed, { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/testslug`);
  assert.equal(res.status, 200);
  const body = Buffer.from(await res.arrayBuffer());
  const text = body.toString('utf8');
  assert.ok(text.startsWith('\x1b[2J\x1b[H'));
  assert.ok(text.includes('\x1bPq'));
  assert.ok(text.includes('\x1b\\'));
  const homes = (text.match(/\x1b\[H/g) || []).length;
  assert.ok(homes >= 2, `expected >=2 frame homes, got ${homes}`);
  assert.ok(text.endsWith('\x1b[0m\n'));
  assert.ok(!text.includes('\x1b[?1049h'));
  assert.ok(!text.includes('\x1b[?25l'));
  server.close();
});