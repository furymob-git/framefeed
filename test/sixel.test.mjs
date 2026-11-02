import test from 'node:test';
import assert from 'node:assert/strict';
import { encodeSixelFrame, PALETTE_SIZE, paletteColor, quantize } from '../lib/sixel.js';

function rasterFromIndices(width, height, pattern) {
  const pixels = Buffer.alloc(width * height * 2);
  const expected = [];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = pattern(x, y);
      const [r, g, b] = paletteColor(idx);
      // pack to RGB565 little-endian
      const v = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
      const off = (y * width + x) * 2;
      pixels[off] = v & 0xff;
      pixels[off + 1] = (v >> 8) & 0xff;
      // expected after RGB565 roundtrip
      const r2 = ((v >> 11) & 31) * 255 / 31 | 0;
      // use same conversion as lib
      const rr = ((v >> 11) & 31);
      const gg = ((v >> 5) & 63);
      const bb = (v & 31);
      const r8 = (rr << 3) | (rr >> 2);
      const g8 = (gg << 2) | (gg >> 4);
      const b8 = (bb << 3) | (bb >> 2);
      expected.push(quantize(r8, g8, b8));
    }
  }
  return { buf: pixels, expected };
}

function number(text, start) {
  const bytes = Buffer.from(text, 'latin1');
  let index = start;
  let value = 0;
  while (index < bytes.length && bytes[index] >= 48 && bytes[index] <= 57) {
    value = value * 10 + (bytes[index] - 48);
    index++;
  }
  return [value, index];
}

function paint(character, width, height, x, y, color, grid) {
  const bits = character - 0x3f;
  for (let row = 0; row < 6; row++) {
    if (bits & (1 << row)) {
      if (y + row < height && x < width) {
        grid[(y + row) * width + x] = color;
      }
    }
  }
}

function decode(bytes, width, height) {
  const text = Buffer.from(bytes).toString('latin1');
  let t = text;
  if (t.startsWith('\x1b[H')) t = t.slice('\x1b[H'.length);
  const body = t.startsWith('\x1bPq') ? t.slice('\x1bPq'.length) : t;
  const suf = '\x1b\\';
  const body2 = body.endsWith(suf) ? body.slice(0, body.length - suf.length) : body;
  const raw = Buffer.from(body2, 'latin1');
  let index = 0;
  if (body2.startsWith('"')) {
    while (index < raw.length && raw[index] !== 0x23) index++;
  }
  const grid = new Array(width * height).fill(255);
  let color = 0;
  let x = 0;
  let y = 0;
  while (index < raw.length) {
    const c = raw[index];
    if (c === 0x23) {
      const [value, next1] = number(body2, index + 1);
      color = value;
      let next = next1;
      while (next < raw.length && (raw[next] === 0x3b || (raw[next] >= 48 && raw[next] <= 57))) {
        next++;
      }
      index = next;
    } else if (c === 0x21) {
      const [count, next] = number(body2, index + 1);
      index = next;
      const character = raw[index];
      index++;
      for (let i = 0; i < count; i++) {
        paint(character, width, height, x, y, color, grid);
        x++;
      }
    } else if (c === 0x24) {
      x = 0;
      index++;
    } else if (c === 0x2d) {
      y += 6;
      x = 0;
      index++;
    } else {
      paint(c, width, height, x, y, color, grid);
      x++;
      index++;
    }
  }
  return grid;
}

test('header and terminator', () => {
  const { buf } = rasterFromIndices(10, 7, (x, y) => (x + y) % 5);
  const out = encodeSixelFrame(buf, 10, 7);
  const text = out.toString('latin1');
  assert.ok(text.startsWith('\x1b[H\x1bPq"1;1;10;7'));
  assert.ok(text.endsWith('\x1b\\'));
  assert.equal((text.match(/-/g) || []).length, 2);
  for (const i of [0, PALETTE_SIZE - 1]) {
    assert.ok(text.includes(`#${i};2;`));
  }
});

test('all 240 palette entries present', () => {
  const { buf } = rasterFromIndices(2, 2, () => 0);
  const out = encodeSixelFrame(buf, 2, 2);
  const text = out.toString('latin1');
  for (let i = 0; i < PALETTE_SIZE; i++) {
    assert.ok(text.includes(`#${i};2;`), `missing palette ${i}`);
  }
});

test('roundtrip quantised pixels', () => {
  const { buf, expected } = rasterFromIndices(10, 7, (x, y) => {
    const m = Math.floor(x / 3) + y;
    if (m % 3 === 0) return 0;
    if (m % 3 === 1) return 7;
    return 100;
  });
  const out = encodeSixelFrame(buf, 10, 7);
  const grid = decode(out, 10, 7);
  assert.deepEqual(grid, expected);
});

test('RLE run for uniform band', () => {
  const { buf } = rasterFromIndices(64, 6, () => 100);
  const out = encodeSixelFrame(buf, 64, 6);
  const text = out.toString('latin1');
  assert.ok(text.includes('!64'), 'should RLE');
});

test('palette endpoints consistent', () => {
  assert.deepEqual(paletteColor(0), [0, 0, 0]);
  assert.deepEqual(paletteColor(215), [255, 255, 255]);
  assert.deepEqual(paletteColor(239), [255, 255, 255]);
  assert.equal(quantize(255, 255, 255), 239);
  assert.equal(quantize(0, 0, 0), 216);
});

test('invalid input returns empty', () => {
  assert.equal(encodeSixelFrame(Buffer.alloc(0), 0, 0).length, 0);
  assert.equal(encodeSixelFrame(Buffer.alloc(4), 2, 2).length, 0);
  assert.equal(encodeSixelFrame(Buffer.alloc(10), -1, 2).length, 0);
  assert.equal(encodeSixelFrame(Buffer.alloc(10), 2, 0).length, 0);
});

test('benchmark 800x450', () => {
  const w = 800, h = 450;
  const buf = Buffer.alloc(w * h * 2);
  for (let i = 0; i < buf.length; i++) buf[i] = (i * 37) & 0xff;
  const start = process.hrtime.bigint();
  const out = encodeSixelFrame(buf, w, h);
  const ms = Number(process.hrtime.bigint() - start) / 1e6;
  console.log(`sixel benchmark: 800x450 in ${ms.toFixed(2)} ms (output ${out.length} bytes)`);
  assert.ok(out.length > 0);
  assert.ok(ms >= 0);
});
