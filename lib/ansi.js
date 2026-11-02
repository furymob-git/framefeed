const HALF_BLOCK = Buffer.from('\u2580');
const RESET_ROW = Buffer.from('\x1b[0m\r\n');
const PREAMBLE = Buffer.from('\x1b[H');

const FG_CACHE = new Array(65536);
const BG_CACHE = new Array(65536);

function rgb565ToRgb(v) {
  const r5 = (v >> 11) & 31;
  const g6 = (v >> 5) & 63;
  const b5 = v & 31;
  return [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2)];
}

function fgSgr(v) {
  let b = FG_CACHE[v];
  if (!b) {
    const [r, g, bb] = rgb565ToRgb(v);
    b = Buffer.from(`\x1b[38;2;${r};${g};${bb}m`);
    FG_CACHE[v] = b;
  }
  return b;
}

function bgSgr(v) {
  let b = BG_CACHE[v];
  if (!b) {
    const [r, g, bb] = rgb565ToRgb(v);
    b = Buffer.from(`\x1b[48;2;${r};${g};${bb}m`);
    BG_CACHE[v] = b;
  }
  return b;
}

function read16(buf, width, x, y) {
  const offset = (y * width + x) * 2;
  return (buf[offset] | (buf[offset + 1] << 8)) & 0xffff;
}

export function encodeAnsiFrame(rgb565Buffer, width, height) {
  const buf = Buffer.isBuffer(rgb565Buffer)
    ? rgb565Buffer
    : Buffer.from(rgb565Buffer || []);
  if (!width || !height || width <= 0 || height <= 0) {
    return Buffer.alloc(0);
  }
  const pieces = [];
  pieces.push(PREAMBLE);
  let prevFg = -1;
  let prevBg = -1;

  let y = 0;
  while (y < height) {
    for (let x = 0; x < width; x++) {
      const topKey = read16(buf, width, x, y);
      const bottomKey =
        y + 1 < height ? read16(buf, width, x, y + 1) : 0;
      if (prevFg !== topKey) {
        pieces.push(fgSgr(topKey));
        prevFg = topKey;
      }
      if (prevBg !== bottomKey) {
        pieces.push(bgSgr(bottomKey));
        prevBg = bottomKey;
      }
      pieces.push(HALF_BLOCK);
    }
    pieces.push(RESET_ROW);
    prevFg = -1;
    prevBg = -1;
    y += 2;
  }
  return Buffer.concat(pieces);
}