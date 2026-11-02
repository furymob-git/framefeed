import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { createVideosHandler } from '../lib/videos.js';

test('videos 200 json', async () => {
  const listVideos = async () => [{ slug: 'a', title: 'A' }];
  const handler = createVideosHandler({ listVideos, maxSeconds: 285 });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/api/videos`);
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('content-type'), 'application/json');
  assert.match(res.headers.get('cache-control') || '', /no-store/);
  const body = await res.json();
  assert.deepEqual(body, { videos: [{ slug: 'a', title: 'A' }], maxSeconds: 285 });
  server.close();
});

test('videos 503 on error', async () => {
  const listVideos = async () => { throw new Error('boom'); };
  const handler = createVideosHandler({ listVideos, maxSeconds: 285 });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/`);
  assert.equal(res.status, 503);
  const body = await res.json();
  assert.deepEqual(body, { error: 'catalog unavailable' });
  server.close();
});

test('videos 405', async () => {
  const handler = createVideosHandler({ listVideos: async () => [], maxSeconds: 285 });
  const server = http.createServer(handler);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  const res = await fetch(`http://127.0.0.1:${port}/`, { method: 'POST' });
  assert.equal(res.status, 405);
  server.close();
});