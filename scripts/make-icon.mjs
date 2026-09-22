import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

// P10-02 app icon generator — deterministic, dependency-free raster of the
// Dark Purple mark (open book + download arrow + emerald verified dot).
// Source of truth for src-tauri/icons via `npx tauri icon <output>`.

const SIZE = 1024;
const BG = [0x10, 0x10, 0x16]; // --background
const PANEL = [0x20, 0x20, 0x2c]; // --surface
const VIOLET = [0xa7, 0x8b, 0xfa]; // --primary
const VIOLET_DEEP = [0x8b, 0x5c, 0xf6]; // --primary-hover
const EMERALD = [0x34, 0xd3, 0x99]; // --success

const rect = (x, y, l, t, r, b) => x >= l && x < r && y >= t && y < b;
const roundRect = (x, y, l, t, r, b, rad) => {
  if (!rect(x, y, l, t, r, b)) return false;
  const cx = Math.min(Math.max(x, l + rad), r - rad);
  const cy = Math.min(Math.max(y, t + rad), b - rad);
  return (x - cx) ** 2 + (y - cy) ** 2 <= rad * rad;
};

function pixel(x, y) {
  if (roundRect(x, y, 32, 32, 992, 992, 208)) {
    // panel background
    let c = PANEL;
    // download arrow (shaft + head), pointing into the book
    const shaft = rect(x, y, 468, 176, 556, 452);
    const headHalf = 150 - (y - 448) * 0.55;
    const head = y >= 448 && y < 560 && Math.abs(x - 512) <= headHalf;
    if (shaft || head) c = VIOLET_DEEP;
    // open book: two pages with a spine gap, slight page step
    const leftPage = rect(x, y, 232, 592, 496, 812);
    const rightPage = rect(x, y, 528, 592, 792, 812);
    const spineShade = rect(x, y, 496, 592, 528, 812);
    if (leftPage || rightPage) c = VIOLET;
    if (spineShade) c = [0x2a, 0x2a, 0x3a];
    // page text lines
    const line =
      (y % 56 < 12 && ((y > 640 && y < 780) || (y > 648 && y < 788))) &&
      ((x > 268 && x < 460) || (x > 564 && x < 756));
    if (line) c = [0x6d, 0x5a, 0xb5];
    // verified dot
    if ((x - 800) ** 2 + (y - 784) ** 2 <= 46 ** 2) c = EMERALD;
    return c;
  }
  return BG;
}

// --- minimal PNG writer (RGBA8, no filter) ---
const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};

const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
let p = 0;
for (let y = 0; y < SIZE; y++) {
  raw[p++] = 0; // filter: none
  for (let x = 0; x < SIZE; x++) {
    const [r, g, b] = pixel(x, y);
    raw[p++] = r;
    raw[p++] = g;
    raw[p++] = b;
    raw[p++] = 255;
  }
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8;
ihdr[9] = 6; // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);
mkdirSync("assets", { recursive: true });
writeFileSync("assets/app-icon-1024.png", png);
console.log(`assets/app-icon-1024.png (${png.length} bytes)`);
