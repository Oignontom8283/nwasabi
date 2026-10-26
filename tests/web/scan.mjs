// La recherche de valeurs de l'interface web (web/src/scan.ts), sur une mémoire fabriquée.
//
//   node tests/web/scan.mjs
//
// Le module est écrit en TypeScript : on le transpile (avec le TypeScript de web/) dans un dossier temporaire avant de le charger.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const source = readFileSync(join(root, "web", "src", "scan.ts"), "utf8");
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-scan-")), "scan.mjs");
writeFileSync(out, outputText);
const { Scan, SIZE, decode, encode, formatValue, matches, parseValue, tile, valueBytes } = await import(pathToFileURL(out).href);

const BASE = 0x2000_0000;
const memory = new Uint8Array(150_000); // plus d'une tranche de lecture (64 Kio) : on traverse les limites
const read = async (address, length) => {
  const at = address - BASE;
  return at < 0 || at + length > memory.length ? null : memory.slice(at, at + length);
};
const put = (offset, value, type) => memory.set(encode(value, type), offset);
const regions = [{ base: BASE, size: memory.length }];
const ok = (name) => console.log(`ok   ${name}`);

// Les valeurs : écriture, lecture, saisie, affichage.
for (const [type, value] of [["u8", 200], ["i8", -5], ["u16", 65000], ["i16", -300], ["u32", 4_000_000_000], ["i32", -70000], ["f32", 1.5], ["f64", -2.25]]) {
  const bytes = encode(value, type);
  assert.equal(bytes.length, SIZE[type]);
  assert.equal(decode(new DataView(bytes.buffer), 0, type), value, type);
}
ok("encode puis decode redonnent la valeur, pour chaque type");

assert.equal(parseValue("42", "u8"), 42);
assert.equal(parseValue("0xff", "u8"), 255);
assert.equal(parseValue("256", "u8"), null);
assert.equal(parseValue("-1", "u16"), null);
assert.equal(parseValue("-1", "i16"), -1);
assert.equal(parseValue("1,5", "f32"), 1.5);
assert.equal(parseValue("abc", "i32"), null);
assert.equal(parseValue("", "u32"), null);
assert.equal(formatValue(0.10000000149011612, "f32"), "0.1");
ok("les valeurs saisies sont validées pour leur type");

// Première recherche : une valeur placée à trois endroits, dont un à cheval sur deux tranches.
put(100, 12345, "u32");
put(65_536 - 2, 12345, "u32"); // aligné sur 2 seulement
put(70_000, 12345, "u32");
let scan = new Scan();
await scan.first(regions, read, { compare: "exact", type: "u32", a: 12345 });
assert.deepEqual(scan.addresses, [BASE + 100, BASE + 70_000], "alignée : l'adresse sur deux octets n'est pas vue");
await scan.first(regions, read, { compare: "exact", type: "u32", a: 12345 }, false);
assert.deepEqual(scan.addresses, [BASE + 100, BASE + 65_534, BASE + 70_000], "non alignée : la valeur à cheval sur deux tranches est trouvée");
ok("première recherche exacte, avec ou sans alignement, à travers les tranches");

// Recherche suivante : on change deux des trois valeurs, on affine.
put(100, 777, "u32");
put(70_000, 12346, "u32");
await scan.next(read, { compare: "exact", type: "u32", a: 777 });
assert.deepEqual(scan.addresses, [BASE + 100]);
assert.deepEqual(scan.previous, [12345]);
assert.deepEqual(scan.values, [777]);
ok("la recherche suivante ne garde que ce qui a la nouvelle valeur");

// Valeur initiale inconnue, puis « a changé » / « a augmenté ».
memory.fill(0);
put(8, 10, "u8");
put(9, 10, "u8");
put(10, 10, "u8");
scan = new Scan();
await scan.first(regions, read, { compare: "unknown", type: "u8" });
assert.equal(scan.count, memory.length, "toutes les adresses sont gardées");
put(8, 11, "u8"); // augmente
put(9, 9, "u8"); // diminue
await scan.next(read, { compare: "changed", type: "u8" });
assert.deepEqual(scan.addresses, [BASE + 8, BASE + 9]);
put(8, 12, "u8");
await scan.next(read, { compare: "increased", type: "u8" });
assert.deepEqual(scan.addresses, [BASE + 8]);
await scan.next(read, { compare: "unchanged", type: "u8" });
assert.deepEqual(scan.addresses, [BASE + 8], "inchangée depuis la recherche d'avant");
ok("valeur inconnue puis modifiée, augmentée, inchangée");

// Comparaisons simples et flottants.
assert.ok(matches({ compare: "between", type: "i16", a: 10, b: -10 }, 0));
assert.ok(!matches({ compare: "between", type: "i16", a: 10, b: -10 }, 11));
assert.ok(matches({ compare: "greater", type: "u8", a: 5 }, 6));
assert.ok(!matches({ compare: "less", type: "u8", a: 5 }, 6));
assert.ok(!matches({ compare: "changed", type: "u8" }, 3), "sans valeur précédente, rien n'a changé");
assert.ok(matches({ compare: "exact", type: "f32", a: 0.1 }, Math.fround(0.1)));
assert.ok(!matches({ compare: "exact", type: "f32", a: 0.1 }, Number.NaN));
memory.fill(0);
put(40, 3.75, "f32");
scan = new Scan();
await scan.first(regions, read, { compare: "exact", type: "f32", a: 3.75 });
assert.deepEqual(scan.addresses, [BASE + 40]);
ok("comparaisons, flottants 32 bits");

// La limite de résultats arrête la recherche sans planter.
memory.fill(0);
scan = new Scan(100);
await scan.first(regions, read, { compare: "exact", type: "u8", a: 0 });
assert.equal(scan.count, 100);
assert.ok(scan.truncated);
ok("la limite de résultats est respectée et signalée");

// Valeurs saisies pour l'écriture : décimal ou hexadécimal, petit ou gros boutiste, répétition sur une zone.
const hex = (bytes) => [...bytes].map((b) => b.toString(16).padStart(2, "0")).join(" ");
assert.equal(hex(valueBytes("u32", "dec", "305419896")), "78 56 34 12");
assert.equal(hex(valueBytes("u32", "hex", "0x12345678")), "78 56 34 12", "l'hexadécimal se lit poids fort d'abord");
assert.equal(hex(valueBytes("u32", "hex", "12345678", true)), "12 34 56 78", "gros-boutiste : même ordre que le texte");
assert.equal(hex(valueBytes("u16", "hex", "ff")), "ff 00", "moins de chiffres que d'octets : complété par des zéros de poids fort");
assert.equal(hex(valueBytes("i16", "dec", "-2")), "fe ff");
assert.equal(hex(valueBytes("f32", "dec", "1,5")), "00 00 c0 3f");
assert.equal(hex(valueBytes("f32", "hex", "3fc00000")), "00 00 c0 3f", "octets bruts d'un flottant");
assert.equal(hex(valueBytes("u8", "dec", "255")), "ff");
assert.equal(valueBytes("u8", "dec", "256"), null);
assert.equal(valueBytes("u8", "hex", "100"), null, "plus de chiffres que d'octets");
assert.equal(valueBytes("u32", "hex", "xyz"), null);
assert.equal(hex(tile(Uint8Array.of(1, 2, 3), 7)), "01 02 03 01 02 03 01", "répété et coupé à la longueur de la zone");
ok("valeurs saisies : décimal, hexadécimal, ordre des octets, répétition");
