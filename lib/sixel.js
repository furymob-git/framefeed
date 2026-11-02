export const PALETTE_SIZE = 240;
const CUBE_STEPS = 6;
const CUBE_ENTRIES = 216;
const GREY_ENTRIES = 24;

const INDEX_CACHE = new Uint8Array(65536);
INDEX_CACHE.fill(255);

function rgb565ToRgb(v) {
  const r5 = (v >> 11) & 31;
  const g6 = (v >> 5) & 63;
  const b5 = v & 31;
  return [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2)];
}

function quantizeRgb(r, g, b) {
  const max = r > g ? (r > b ? r : b) : (g > b ? g : b);
  const min = r < g ? (r < b ? r : b) : (g < b ? g : b);
  if (max - min < 12) {
    const luma = (r * 77 + g * 150 + b * 29) >> 8;
    let step = Math.floor((luma * (GREY_ENTRIES - 1) + 127) / 255);
    if (step > GREY_ENTRIES - 1) step = GREY_ENTRIES - 1;
    return CUBE_ENTRIES + step;
  }
  const ri = Math.floor((r * (CUBE_STEPS - 1) + 127) / 255);
  const gi = Math.floor((g * (CUBE_STEPS - 1) + 127) / 255);
  const bi = Math.floor((b * (CUBE_STEPS - 1) + 127) / 255);
  return ri * 36 + gi * 6 + bi;
}

function quantize565(v) {
  const cached = INDEX_CACHE[v];
  if (cached !== 255) return cached;
  const rgb = rgb565ToRgb(v);
  const idx = quantizeRgb(rgb[0], rgb[1], rgb[2]);
  INDEX_CACHE[v] = idx;
  return idx;
}

export function paletteColor(index) {
  if (index < CUBE_ENTRIES) {
    const r = Math.floor(index / 36) * 51;
    const g = Math.floor((index % 36) / 6) * 51;
    const b = (index % 6) * 51;
    return [r, g, b];
  }
  const step = index - CUBE_ENTRIES;
  const level = Math.floor((step * 255) / (GREY_ENTRIES - 1));
  return [level, level, level];
}

export function quantize(r, g, b) {
  return quantizeRgb(r, g, b);
}

function percent(value) {
  return Math.floor((value * 100 + 127) / 255);
}

// Reusable scratch buffers.
let scratchPresent = new Uint8Array(PALETTE_SIZE);
let scratchColors = [];
let scratchColorBits = null;
let scratchColorBitsW = 0;
let scratchFirst = new Int32Array(PALETTE_SIZE);
let scratchLast = new Int32Array(PALETTE_SIZE);

export function encodeSixelFrame(rgb565Buffer, width, height) {
  const buf = Buffer.isBuffer(rgb565Buffer)
    ? rgb565Buffer
    : Buffer.from(rgb565Buffer || []);
  const w = width | 0;
  const h = height | 0;
  if (w <= 0 || h <= 0) return Buffer.alloc(0);
  if (buf.length < w * h * 2) return Buffer.alloc(0);

  if (scratchColorBitsW !== w) {
    scratchColorBits = new Uint8Array(PALETTE_SIZE * w);
    scratchColorBitsW = w;
  }

  const pieces = [];
  pieces.push('\x1b[H');
  pieces.push('\x1bPq');
  pieces.push(`"1;1;${w};${h}`);

  for (let i = 0; i < PALETTE_SIZE; i++) {
    const pc = paletteColor(i);
    pieces.push(`#${i};2;${percent(pc[0])};${percent(pc[1])};${percent(pc[2])}`);
  }

  const present = scratchPresent;
  const colors = scratchColors;
  const colorBits = scratchColorBits;
  const firstArr = scratchFirst;
  const lastArr = scratchLast;

  let y = 0;
  while (y < h) {
    const rows = h - y < 6 ? h - y : 6;
    present.fill(0);
    colorBits.fill(0);
    for (let i = 0; i < PALETTE_SIZE; i++) {
      firstArr[i] = w;
      lastArr[i] = -1;
    }

    for (let row = 0; row < rows; row++) {
      const start = (y + row) * w * 2;
      const bit = 1 << row;
      for (let x = 0; x < w; x++) {
        const off = start + x * 2;
        const v = buf[off] | (buf[off + 1] << 8);
        const idx = quantize565(v);
        present[idx] = 1;
        const p = idx * w + x;
        colorBits[p] |= bit;
        if (x < firstArr[idx]) firstArr[idx] = x;
        if (x > lastArr[idx]) lastArr[idx] = x;
      }
    }

    colors.length = 0;
    for (let i = 0; i < PALETTE_SIZE; i++) {
      if (present[i]) colors.push(i);
    }

    for (let pos = 0; pos < colors.length; pos++) {
      const color = colors[pos];
      pieces.push(`#${color}`);
      const base = color * w;
      let first = firstArr[color];
      let last = lastArr[color];
      if (first > 0) {
        pieces.push(`!${first}?`);
      }
      if (first === -1 || first === w) {
        pieces.push(pos + 1 < colors.length ? '$' : '-');
        continue;
      }
      let x = first;
      while (x <= last) {
        const bitsv = colorBits[base + x];
        let run = 1;
        while (x + run <= last && colorBits[base + x + run] === bitsv) run++;
        if (run > 3) {
          pieces.push(`!${run}`);
          pieces.push(String.fromCharCode(0x3F + bitsv));
        } else {
          const ch = String.fromCharCode(0x3F + bitsv);
          for (let i = 0; i < run; i++) pieces.push(ch);
        }
        x += run;
      }
      pieces.push(pos + 1 < colors.length ? '$' : '-');
    }

    y += 6;
  }

  pieces.push('\x1b\\');
  return Buffer.from(pieces.join(''), 'latin1');
}