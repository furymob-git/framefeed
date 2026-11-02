import test from 'node:test';
import assert from 'node:assert/strict';
import { encodeAnsiFrame } from '../lib/ansi.js';

test('ansi golden 2x2', () => {
  const buf = Buffer.from([0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00, 0xff, 0xff]);
  const out = encodeAnsiFrame(buf, 2, 2);
  const expected = '\x1b[H\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀\x1b[38;2;0;255;0m\x1b[48;2;255;255;255m▀\x1b[0m\r\n';
  assert.equal(out.toString('utf8'), expected);
});

test('ansi odd height pads last row with black', () => {
  // 1x3 pixels: (1,2,3),(4,5,6),(7,8,9) as RGB888? Here RGB565 arbitrary; just check row count and black bg.
  const buf = Buffer.from([1, 0, 2, 0, 3, 0]);
  const out = encodeAnsiFrame(buf, 1, 3);
  const text = out.toString('utf8');
  assert.equal((text.match(/\r\n/g) || []).length, 2);
  assert.ok(text.includes('\x1b[48;2;0;0;0m'));
});

test('ansi empty raster is no-op', () => {
  const out = encodeAnsiFrame(Buffer.alloc(0), 0, 0);
  assert.equal(out.length, 0);
});

test('ansi benchmark 100 frames 80x48', () => {
  const frameBytes = 80 * 48 * 2;
  const buf = Buffer.alloc(frameBytes * 100);
  const start = process.hrtime.bigint();
  for (let i = 0; i < 100; i++) {
    encodeAnsiFrame(buf.subarray(i * frameBytes, (i + 1) * frameBytes), 80, 48);
  }
  const ms = Number(process.hrtime.bigint() - start) / 1e6;
  console.log(`ansi benchmark: 100 frames 80x48 in ${ms.toFixed(2)} ms`);
  assert.ok(ms >= 0);
});