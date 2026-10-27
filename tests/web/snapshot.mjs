// Comparaison d'instantanés de la mémoire (web/src/snapshot.ts).
//
//   node tests/web/snapshot.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "snapshot.ts"), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-snap-")), "snapshot.mjs");
writeFileSync(out, outputText);
const { diffBytes, compare } = await import(pathToFileURL(out).href);

const a = new Uint8Array(64);
const b = a.slice();
b[3] = 1; b[5] = 2; // proches : une seule plage
b[40] = 9; // loin : une autre
const changes = diffBytes(0x2000_0000, a, b);
assert.equal(changes.length, 2);
assert.deepEqual([changes[0].address, changes[0].length], [0x2000_0003, 3]);
assert.deepEqual([...changes[0].before], [0, 0, 0]);
assert.deepEqual([...changes[0].after], [1, 0, 2]);
assert.deepEqual([changes[1].address, changes[1].length], [0x2000_0028, 1]);
assert.deepEqual(diffBytes(0, a, a), []);
// Dernier octet différent : la plage est bien refermée.
const c = a.slice(); c[63] = 7;
assert.deepEqual(diffBytes(0, a, c).map((x) => [x.address, x.length]), [[63, 1]]);
// Deux zones.
const snap = { takenAt: 0, parts: [{ base: 0x100, bytes: a }, { base: 0x900, bytes: a }] };
const all = compare(snap, [{ base: 0x100, bytes: a }, { base: 0x900, bytes: c }]);
assert.deepEqual(all.map((x) => x.address), [0x900 + 63]);
console.log("ok   instantanés : plages regroupées, zones multiples, dernier octet");
