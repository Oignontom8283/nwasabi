// Lecture des registres de faute (web/src/faultRegs.ts).
//
//   node tests/web/faultregs.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "faultRegs.ts"), "utf8"), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
const out = join(mkdtempSync(join(tmpdir(), "nwasabi-faultregs-")), "faultRegs.mjs");
writeFileSync(out, outputText);
const { CFSR_BITS, HFSR_BITS, setBits, hasFault, SCB } = await import(pathToFileURL(out).href);

assert.deepEqual(setBits((1 << 9) | (1 << 15), CFSR_BITS).map((b) => b.name), ["PRECISERR", "BFARVALID"]);
assert.deepEqual(setBits(1 << 24, CFSR_BITS).map((b) => b.name), ["UNALIGNED"]);
assert.deepEqual(setBits(0x8000_0000 | (1 << 30), HFSR_BITS).map((b) => b.name), ["FORCED"]);
assert.deepEqual(setBits(0, CFSR_BITS), []);
assert.equal(hasFault({ cfsr: 0, hfsr: 0, dfsr: 0, mmfar: 0, bfar: 0 }), false);
assert.equal(hasFault({ cfsr: 0, hfsr: 1 << 30, dfsr: 0, mmfar: 0, bfar: 0 }), true);
assert.equal(SCB.CFSR, 0xe000ed28);
console.log("ok   registres de faute : bits levés, absence de faute, adresses");
