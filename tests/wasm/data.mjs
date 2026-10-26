// Une application qui réclame des données externes n'est pas lancée : le lecteur les attend (`needsData`), puis
// les reçoit ou s'en passe. Utilise lua.nwa du corpus ; sans corpus, le test est ignoré.
//
//   node tests/wasm/data.mjs [dossier de .nwa]

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Player } from "../../packages/js/dist/index.js";
import { corpusDir } from "./corpus.mjs";

const corpus = corpusDir();
const file = corpus && join(corpus, "lua.nwa");
if (!file || !existsSync(file)) process.exit(0);

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const wasm = readFileSync(process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm"));
const scheduler = { now: () => 0, request: () => 0, cancel() {} };
const nwa = new Uint8Array(readFileSync(file));
let failures = 0;
const check = async (name, fn) => {
  try {
    await fn();
    console.log(`ok   ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL ${name}\n     ${error.stack ?? error}`);
  }
};

await check("sans données, l'application n'est pas lancée et le lecteur les attend", async () => {
  const player = await Player.create(wasm, { scheduler });
  assert.equal(player.load(nwa, "lua.nwa"), false);
  assert.equal(player.state.needsData, true);
  assert.equal(player.state.loaded, false);
  assert.equal(player.state.error, "");
});

await check("avec des données, elle se lance", async () => {
  const player = await Player.create(wasm, { scheduler });
  player.load(nwa, "lua.nwa");
  assert.equal(player.load(nwa, "lua.nwa", new TextEncoder().encode("print(1)")), true);
  assert.equal(player.state.needsData, false);
  assert.equal(player.state.loaded, true);
});

await check("« lancer sans données » la lance avec un jeu vide", async () => {
  const player = await Player.create(wasm, { scheduler });
  player.load(nwa, "lua.nwa");
  assert.equal(player.runWithoutData(), true);
  assert.equal(player.state.needsData, false);
  assert.equal(player.state.loaded, true);
});

if (failures) process.exit(1);
