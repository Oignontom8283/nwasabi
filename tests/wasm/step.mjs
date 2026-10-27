// L'avance image par image du lecteur (en pause), avec une application de test committée (pas de corpus nécessaire).
//
//   node tests/wasm/step.mjs

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Player } from "../../packages/js/dist/index.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const wasm = readFileSync(process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm"));
const nwa = new Uint8Array(readFileSync(join(root, "tests/apps/bin/keys.nwa")));

const player = await Player.create(wasm, { scheduler: { now: () => 0, request: () => 0, cancel: () => {} } });
assert.equal(player.stepFrame(), false, "sans application, rien n'avance");
assert.ok(player.load(nwa, "keys"));
player.stop();
const frames = player.state.frames;
const time = player.time();
assert.equal(player.stepFrame(), true);
assert.equal(player.state.frames, frames + 1, "une image de plus");
const elapsed = player.time() - time;
assert.ok(elapsed >= 1000 / 60 - 0.5 && elapsed < 40, `1/60 s passe pour l'application (${elapsed} ms)`);
player.stepFrame();
player.stepFrame();
assert.equal(player.state.frames, frames + 3, "trois images après trois pas");
assert.equal(player.state.running, false, "la simulation reste en pause");
console.log("ok   l'avance image par image : une image et 1/60 s par pas, simulation en pause");
