// L'enregistreur GIF de l'interface web (web/src/gif.ts) : la compression LZW est vérifiée par une décompression, et la
// structure du fichier (en-tête, palette, images, durées) par une lecture des blocs.
//
//   node tests/web/gif.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "gif.ts"), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-gif-")), "gif.mjs");
writeFileSync(out, outputText);
const { GifRecorder, lzw } = await import(pathToFileURL(out).href);

/** Décompression LZW de GIF (pour vérifier `lzw`). */
function unlzw(blocks, minCodeSize = 8) {
  const data = [];
  for (let i = 0; blocks[i] !== 0; i += blocks[i] + 1) data.push(...blocks.slice(i + 1, i + 1 + blocks[i]));
  const clear = 1 << minCodeSize, end = clear + 1;
  let size = minCodeSize + 1, bits = 0, acc = 0, pos = 0;
  let table = [];
  const reset = () => { table = Array.from({ length: clear }, (_, i) => [i]); table.push(null, null); size = minCodeSize + 1; };
  reset();
  const result = [];
  let previous = null;
  while (true) {
    while (bits < size && pos < data.length) { acc |= data[pos++] << bits; bits += 8; }
    if (bits < size) break;
    const code = acc & ((1 << size) - 1); acc >>>= size; bits -= size;
    if (code === clear) { reset(); previous = null; continue; }
    if (code === end) break;
    let entry;
    if (code < table.length) entry = table[code];
    else entry = [...previous, previous[0]];
    result.push(...entry);
    if (previous) table.push([...previous, entry[0]]);
    if (table.length === 1 << size && size < 12) size++;
    previous = entry;
  }
  return Uint8Array.from(result);
}

// LZW : aller-retour sur des données variées, dont assez longues pour remplir le dictionnaire (4096 codes).
const samples = [
  Uint8Array.of(1), Uint8Array.of(7, 7, 7, 7, 7, 7, 7, 7), Uint8Array.from({ length: 1000 }, (_, i) => i % 7),
  Uint8Array.from({ length: 76_800 }, (_, i) => ((i * 2654435761) >>> 24) & 0xff),
  Uint8Array.from({ length: 76_800 }, (_, i) => (i >> 8) & 0xff),
];
for (const data of samples) assert.deepEqual(unlzw(lzw(data)), data, `aller-retour LZW (${data.length} octets)`);
console.log("ok   LZW : décompression identique à l'original, dictionnaire plein compris");

// Un enregistrement : 3 images dont deux identiques (fusionnées), palette et durées.
const w = 4, h = 2;
const frame = (colors) => Uint8ClampedArray.from(colors.flatMap((c) => [...c, 255]));
const red = [255, 0, 0], blue = [0, 0, 255], white = [255, 255, 255];
const recorder = new GifRecorder(w, h, 40);
recorder.add(frame([red, red, white, white, blue, blue, white, white]), 0);
recorder.add(frame([red, red, white, white, blue, blue, white, white]), 40); // identique : durée rallongée
recorder.add(frame([blue, blue, white, white, red, red, white, white]), 120); // différente
const gif = recorder.finish();
assert.equal(new TextDecoder().decode(gif.slice(0, 6)), "GIF89a");
assert.equal(gif[6] | (gif[7] << 8), w);
assert.equal(gif[8] | (gif[9] << 8), h);
assert.equal(gif[gif.length - 1], 0x3b, "fin de fichier");
let at = 13 + 768; // en-tête + palette globale
assert.equal(gif[at], 0x21); at += 19; // extension de boucle (3 + 11 + 5)
const delays = [];
let images = 0;
while (gif[at] === 0x21 && gif[at + 1] === 0xf9) {
  delays.push(gif[at + 4] | (gif[at + 5] << 8));
  assert.equal(gif[at + 8], 0x2c, "descripteur d'image");
  at += 8 + 10 + 1; // contrôle + descripteur + taille LZW
  const indices = unlzw(gif.slice(at));
  assert.equal(indices.length, w * h);
  while (gif[at] !== 0) at += gif[at] + 1;
  at += 1; images++;
}
assert.equal(images, 2, "l'image répétée est fusionnée");
assert.deepEqual(delays, [12, 4], "durées en centièmes de seconde : 120 ms (deux images fusionnées) puis 40 ms");
console.log("ok   enregistrement : en-tête, palette, images fusionnées, durées");
