// Les paramètres de l'adresse de l'émulateur (web/src/launch.ts) : fichiers à charger, modèle et firmware.
//
//   node tests/web/launch.mjs

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ts = createRequire(join(root, "web", "package.json"))("typescript");
const dir = mkdtempSync(join(tmpdir(), "nwasabi-launch-"));
const { outputText } = ts.transpileModule(readFileSync(join(root, "web", "src", "launch.ts"), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
});
writeFileSync(join(dir, "launch.mjs"), outputText);
const { launchFirmware, launchModel, launchParams, FILE_PARAMS } = await import(pathToFileURL(join(dir, "launch.mjs")).href);

assert.deepEqual([...FILE_PARAMS].sort(), ["data", "flash", "nds", "nfs", "nmk", "nwa"]);

const params = launchParams("?nwa=https://x.org/a.nwa&flash=k.nfd&nds=s.nds&lang=en");
assert.equal(params.nwa, "https://x.org/a.nwa");
assert.equal(params.flash, "k.nfd");
assert.equal(params.nds, "s.nds");
assert.equal(params.data, null);

assert.equal(launchModel("?model=n0120"), "n0120");
assert.equal(launchModel("?model=N0115"), "n0115", "la casse ne compte pas");
assert.equal(launchModel("?model=n9999"), null, "un modèle inconnu est ignoré");
assert.equal(launchModel("?nwa=a.nwa"), null);

assert.equal(launchFirmware("?firmware=24"), "24");
assert.equal(launchFirmware("?firmware=26.4.0"), "26.4.0");
assert.equal(launchFirmware("?firmware=25"), null);
assert.equal(launchFirmware(""), null);
console.log("launch : ok");
