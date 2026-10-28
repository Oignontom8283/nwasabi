// Les octets placés en flash (`.nfd`) passent par le lecteur : chargés avec l'application, gardés quand
// on la relance, et rendus identiques par un état enregistré puis restauré.
//
//   node tests/wasm/flash.mjs
//
// Utilise l'application de test `integer` du dépôt : pas besoin de corpus.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Player } from "../../packages/js/dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const wasmPath =
  process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const nwa = new Uint8Array(readFileSync(join(root, "tests/apps/bin/integer.nwa")));

const scheduler = { now: () => 0, request: () => 0, cancel: () => {} };
const nfd = {
  nfd: 1,
  active_slot: "a",
  segments: [{ name: "x", address: "0x90400000", content: Buffer.from([1, 2, 3, 4]).toString("base64") }],
};
const read = (player, address) => player.nwasabi.request({ cmd: "memory_read", address, length: 4 }).hex;

const player = await Player.create(readFileSync(wasmPath), { scheduler });
assert.ok(player.load(nwa, "integer.nwa", null, nfd), "chargement avec des octets en flash");
assert.equal(read(player, 0x90400000), "01020304");

player.restart();
assert.equal(read(player, 0x90400000), "01020304", "gardés quand on relance");

player.changeModel("n0120");
assert.equal(read(player, 0x90400000), "01020304", "gardés quand on change de modèle");

player.stop();
const dump = player.dump();
assert.ok(dump?.app?.flash, "l'état enregistré porte les octets");
const other = await Player.create(readFileSync(wasmPath), { scheduler });
assert.ok(other.restore(dump), "restauration");
assert.equal(read(other, 0x90400000), "01020304", "rétablis par la restauration");

// Un document invalide est refusé avec une explication et ne casse pas le lecteur.
assert.equal(player.load(nwa, "integer.nwa", null, { nfd: 1, segments: [{ address: "0x20000000", content: "AQ==" }] }), false);
assert.match(player.state.error ?? "", /flash externe/);
console.log("flash : ok");
