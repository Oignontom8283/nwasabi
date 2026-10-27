// Les calculs de sélection de lignes (web/src/selectionMath.ts).
//
//   node tests/web/selection.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "selectionMath.ts"), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-selection-")), "selectionMath.mjs");
writeFileSync(out, outputText);
const { intersects, intersectsAny, union, rectBetween, rangeKeys, neighbour } = await import(pathToFileURL(out).href);

const a = { x: 0, y: 0, w: 10, h: 10 };
assert.equal(intersects(a, { x: 5, y: 5, w: 10, h: 10 }), true);
assert.equal(intersects(a, { x: 10, y: 0, w: 5, h: 5 }), false, "bords qui se touchent : pas de recouvrement");
assert.equal(intersects(a, { x: 20, y: 0, w: 5, h: 5 }), false);
// Une ligne dont le texte s'arrête à x = 100 : un rectangle à droite du texte ne la prend pas.
const content = union([{ x: 0, y: 0, w: 40, h: 20 }, { x: 50, y: 0, w: 50, h: 20 }]);
assert.deepEqual(content, { x: 0, y: 0, w: 100, h: 20 });
assert.equal(intersects({ x: 300, y: 0, w: 50, h: 50 }, content), false);
assert.equal(intersects({ x: 90, y: 5, w: 50, h: 5 }, content), true);
assert.equal(union([]), null);
// Plusieurs textes dans une ligne : un rectangle dans le trou entre deux textes ne touche aucun d'eux.
const texts = [{ x: 0, y: 0, w: 40, h: 20 }, { x: 80, y: 0, w: 50, h: 20 }];
assert.equal(intersectsAny({ x: 50, y: 5, w: 20, h: 5 }, texts), false, "entre deux textes");
assert.equal(intersectsAny({ x: 30, y: 5, w: 20, h: 5 }, texts), true, "touche le premier texte");
assert.equal(intersectsAny({ x: 120, y: 5, w: 100, h: 5 }, texts), true, "touche le second texte");
assert.equal(intersectsAny({ x: 0, y: 40, w: 100, h: 5 }, texts), false, "sous la ligne");
assert.equal(intersectsAny({ x: 0, y: 0, w: 5, h: 5 }, []), false);
assert.deepEqual(rectBetween({ x: 10, y: 10 }, { x: 4, y: 30 }), { x: 4, y: 10, w: 6, h: 20 });
const keys = [10, 20, 30, 40, 50];
assert.deepEqual(rangeKeys(keys, 20, 40), [20, 30, 40]);
assert.deepEqual(rangeKeys(keys, 40, 20), [20, 30, 40]);
assert.deepEqual(rangeKeys(keys, 20, 99), []);
assert.equal(neighbour(keys, 30, 1), 40);
assert.equal(neighbour(keys, 50, 1), 50, "borné à la fin");
assert.equal(neighbour(keys, 10, -1), 10, "borné au début");
assert.equal(neighbour(keys, null, 1), 10);
assert.equal(neighbour(keys, null, -1), 50);
assert.equal(neighbour([], 1, 1), null);
console.log("ok   sélection de lignes : recouvrement, boîte du contenu, plages, voisines");
