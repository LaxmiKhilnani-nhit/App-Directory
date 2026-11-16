/**
 * Placeholder icon generator.
 *
 * Draws a rounded "tile grid" mark and writes the PNG/ICO sizes Tauri needs, plus the
 * admin portal favicon. Replace the outputs with the real brand icon when it is supplied
 * (PLAN.md §12) — this script exists so the scaffold builds without binary assets in git.
 *
 * Usage: pnpm icons
 */
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// ---------------------------------------------------------------- PNG encoding

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const typeBuffer = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function encodePng(size, pixelAt) {
  const stride = size * 4 + 1;
  const raw = Buffer.alloc(stride * size);

  for (let y = 0; y < size; y += 1) {
    raw[y * stride] = 0; // filter: none
    for (let x = 0; x < size; x += 1) {
      const [r, g, b, a] = pixelAt(x + 0.5, y + 0.5);
      const offset = y * stride + 1 + x * 4;
      raw[offset] = r;
      raw[offset + 1] = g;
      raw[offset + 2] = b;
      raw[offset + 3] = a;
    }
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/** Wraps a PNG in an ICO container (Windows Vista+ supports PNG payloads). */
function encodeIco(png) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(1, 4); // image count

  const entry = Buffer.alloc(16);
  entry[0] = 0; // width 0 == 256
  entry[1] = 0; // height 0 == 256
  entry[2] = 0; // palette colours
  entry[3] = 0; // reserved
  entry.writeUInt16LE(1, 4); // colour planes
  entry.writeUInt16LE(32, 6); // bits per pixel
  entry.writeUInt32BE(0, 8);
  entry.writeUInt32LE(png.length, 8);
  entry.writeUInt32LE(22, 12); // offset: header + one entry

  return Buffer.concat([header, entry, png]);
}

// ---------------------------------------------------------------- artwork

function roundedRectDistance(px, py, cx, cy, halfW, halfH, radius) {
  const dx = Math.abs(px - cx) - (halfW - radius);
  const dy = Math.abs(py - cy) - (halfH - radius);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  const inside = Math.min(Math.max(dx, dy), 0);
  return outside + inside - radius;
}

function coverage(distance) {
  // ~1px anti-aliased edge
  return Math.min(Math.max(0.5 - distance, 0), 1);
}

function mix(a, b, t) {
  return a + (b - a) * t;
}

function makeMark(size) {
  const pad = size * 0.03;
  const bgRadius = size * 0.235;
  const centre = size / 2;

  const gridPad = size * 0.265;
  const gap = size * 0.058;
  const tile = (size - gridPad * 2 - gap) / 2;
  const tileRadius = tile * 0.26;

  const from = [0x2f, 0x6f, 0xed];
  const to = [0x1b, 0x2f, 0x6b];

  return (px, py) => {
    const bgAlpha = coverage(
      roundedRectDistance(px, py, centre, centre, size / 2 - pad, size / 2 - pad, bgRadius),
    );

    let tileAlpha = 0;
    for (let row = 0; row < 2; row += 1) {
      for (let col = 0; col < 2; col += 1) {
        const cx = gridPad + col * (tile + gap) + tile / 2;
        const cy = gridPad + row * (tile + gap) + tile / 2;
        tileAlpha = Math.max(
          tileAlpha,
          coverage(roundedRectDistance(px, py, cx, cy, tile / 2, tile / 2, tileRadius)),
        );
      }
    }
    tileAlpha *= 0.94;

    const t = (px + py) / (2 * size);
    const bg = [mix(from[0], to[0], t), mix(from[1], to[1], t), mix(from[2], to[2], t)];

    const outAlpha = tileAlpha + bgAlpha * (1 - tileAlpha);
    if (outAlpha <= 0) return [0, 0, 0, 0];

    const rgb = [0, 1, 2].map((index) =>
      Math.round(
        (255 * tileAlpha + bg[index] * bgAlpha * (1 - tileAlpha)) / outAlpha,
      ),
    );

    return [rgb[0] ?? 0, rgb[1] ?? 0, rgb[2] ?? 0, Math.round(outAlpha * 255)];
  };
}

// ---------------------------------------------------------------- output

function write(relativePath, buffer) {
  const target = join(ROOT, relativePath);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, buffer);
  console.log(`  ${relativePath}  (${buffer.length.toLocaleString()} bytes)`);
}

const ICON_DIR = "apps/desktop/src-tauri/icons";

const sizes = [
  [32, `${ICON_DIR}/32x32.png`],
  [128, `${ICON_DIR}/128x128.png`],
  [256, `${ICON_DIR}/128x128@2x.png`],
  [512, `${ICON_DIR}/icon.png`],
  [64, "apps/admin/public/favicon.png"],
  [64, "apps/desktop/public/favicon.png"],
];

console.log("Generating placeholder icons…");

for (const [size, target] of sizes) {
  write(target, encodePng(size, makeMark(size)));
}

write(`${ICON_DIR}/icon.ico`, encodeIco(encodePng(256, makeMark(256))));

console.log("Done. Replace these with the real brand icon when available.");
