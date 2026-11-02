import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import zlib from 'node:zlib';
import { createStreamHandler } from '../lib/stream.js';

const SLUG_RE = /^[a-z0-9][a-z0-9-]{0,62}$/;

test('slug regex accepts valid', () => {
  assert.ok(SLUG_RE.test('badapple'));
  assert.ok(SLUG_RE.test('a-b'));
  assert.ok(SLUG_RE.test('abc123'));
});

test('slug regex rejects invalid', () => {
  assert.ok(!SLUG_RE.test('Bad'));
  assert.ok(!SLUG_RE.test('-bad'));
  assert.ok(!SLUG_RE.test('bad_apple'));
  assert.ok(!SLUG_RE.test(''));
});

test('stream slug from query param', async () => {
  const getVideo = async (slug) => (slug === 'abc' ? { slug, title: 't', streamUrl: 'x', widthPx: 80, heightPx: 48, fps: 24, frameCount: 1, durationSeconds: 1 } : null);
  const raw = Buffer.alloc(80*48*2);
  const handler = createStreamHandler({ getVideo, maxSeconds: 285, fetchImpl: async () => new Response(zlib.gzipSync(raw), { status: 200 }) });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/api/stream?slug=abc`);
  assert.equal(res.status, 200);
  server.close();
});