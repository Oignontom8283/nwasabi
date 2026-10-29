// Le débogage du code source sous WebAssembly : mêmes informations DWARF, même pile, mêmes valeurs que le programme natif.
//
//   node tests/wasm/debug.mjs
//
// Utilise l'application de test `debugme` du dépôt (compilée avec -g) : pas besoin de corpus.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Player } from "../../packages/js/dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const wasmPath = process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const nwa = new Uint8Array(readFileSync(join(root, "tests/apps/bin/debugme.nwa")));

const scheduler = { now: () => 0, request: () => 0, cancel: () => {} };
const player = await Player.create(readFileSync(wasmPath), { scheduler });
assert.ok(player.load(nwa, "debugme.nwa"));
player.stop();
const ask = (request) => player.nwasabi.request(request);

const info = ask({ cmd: "debug_info" });
assert.equal(info.available, true);
assert.equal(info.files[0].path, "src/debugme.c");
assert.equal(info.functions, 6);

const stop = ask({ cmd: "break_function", name: "factorial" });
assert.equal(stop.function, "factorial");
assert.equal(ask({ cmd: "run", instructions: 5_000_000 }).stop.kind, "breakpoint");
const frames = ask({ cmd: "frames" }).frames;
assert.deepEqual(frames.slice(0, 2).map((f) => f.function), ["factorial", "main"]);
assert.equal(ask({ cmd: "eval", expr: "n * 2" }).value, "10");
const locals = ask({ cmd: "locals" });
assert.equal(locals.params[0].name, "n");

// Un pas à la ligne et l'ELF exporté.
const step = ask({ cmd: "step_source", mode: "over" });
assert.equal(step.function, "factorial");
const elf = ask({ cmd: "export_elf" });
assert.equal(Buffer.from(elf.elf, "base64").subarray(0, 4).toString("latin1"), "\x7fELF");
console.log("debug : ok");
