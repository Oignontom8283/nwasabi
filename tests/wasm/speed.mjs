// La vitesse du processeur du lecteur : modèle, fréquence personnalisée, sans limite.
//
//   node tests/wasm/speed.mjs <dossier de .nwa>
//
// Comme pour l'horloge, le temps et le planificateur sont simulés : le test est déterministe. L'application
// de test (sierpinski) calcule sans jamais attendre une image : ce qu'elle exécute par image dépend
// donc seulement de la vitesse. Sans dossier (ou sans NWASABI_CORPUS), le test est ignoré.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { DEFAULT_CUSTOM_MHZ, Player } from "../../packages/js/dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const wasmPath =
  process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const corpus = process.argv[2] ?? process.env.NWASABI_CORPUS;
if (!corpus) {
  console.log("corpus absent : test de la vitesse ignoré (donner un dossier ou NWASABI_CORPUS)");
  process.exit(0);
}

// Le temps ne passe pas pendant l'exécution d'une image : seule la limite de fréquence la borne.
let now = 0;
let callbacks = [];
const scheduler = {
  now: () => now,
  request: (callback) => callbacks.push(callback),
  cancel: () => {
    callbacks = [];
  },
};
const FRAME_MS = 1000 / 60;
/** Fait passer `frames` images de 60 Hz. */
function frames(count) {
  for (let i = 0; i < count; i++) {
    now += FRAME_MS;
    const due = callbacks;
    callbacks = [];
    for (const callback of due) callback(now);
  }
}

const file = readdirSync(corpus).find((f) => f.startsWith("sierpinski"));
assert.ok(file, "sierpinski.nwa dans le corpus");
const nwa = new Uint8Array(readFileSync(join(corpus, file)));
const wasm = readFileSync(wasmPath);

/** Instructions exécutées par l'application pendant `count` images, pour un lecteur réglé comme demandé. */
async function executed(count, options = {}, tweak = () => {}) {
  now = 0;
  callbacks = [];
  const player = await Player.create(wasm, { scheduler, slice: 20_000, ...options });
  assert.ok(player.load(nwa, "sierpinski"));
  tweak(player);
  const before = player.state.instructions;
  frames(count);
  player.stop();
  return { player, instructions: player.state.instructions - before };
}

let failures = 0;
const check = async (name, fn) => {
  try {
    await fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failures++;
    console.log(`FAIL ${name}\n     ${e.message}`);
  }
};
const near = (actual, expected, tolerance = 0.03) =>
  assert.ok(
    Math.abs(actual - expected) <= expected * tolerance,
    `${actual} instructions, ${expected} attendues (± ${tolerance * 100} %)`,
  );

await check("l'état de départ : modèle, 192 MHz personnalisés par défaut", async () => {
  const player = await Player.create(wasm, { scheduler });
  assert.equal(player.state.speed, "model");
  assert.equal(player.state.customMhz, DEFAULT_CUSTOM_MHZ);
  assert.equal(DEFAULT_CUSTOM_MHZ, 192);
  assert.equal(player.state.limitMhz, 192, "fréquence du N0110");
});

await check("la fréquence suit le modèle", async () => {
  const player = await Player.create(wasm, { scheduler });
  player.changeModel("n0120");
  assert.equal(player.state.limitMhz, 540);
  player.changeModel("n0115");
  assert.equal(player.state.limitMhz, 192);
});

await check("en mode personnalisé, l'application exécute ce que la fréquence permet", async () => {
  // 10 images (166,7 ms) à 10 MHz : 1,667 million d'instructions ; à 40 MHz : quatre fois plus.
  const slow = await executed(10, { speed: "custom", customMhz: 10 });
  near(slow.instructions, 10 * FRAME_MS * 1000 * 10);
  const fast = await executed(10, { speed: "custom", customMhz: 40 });
  near(fast.instructions, 10 * FRAME_MS * 1000 * 40);
  assert.equal(slow.player.state.limitMhz, 10);
});

await check("le mode modèle applique la fréquence du modèle", async () => {
  const { instructions, player } = await executed(5);
  near(instructions, 5 * FRAME_MS * 1000 * 192);
  assert.equal(player.state.limitMhz, 192);
});

await check("la fréquence personnalisée est ignorée hors du mode personnalisé", async () => {
  const model = await executed(5, { customMhz: 10 });
  near(model.instructions, 5 * FRAME_MS * 1000 * 192);
  assert.equal(model.player.state.customMhz, 10);
});

await check("on peut changer la vitesse et la fréquence en cours d'exécution", async () => {
  const { player, instructions } = await executed(5, { speed: "custom", customMhz: 10 });
  near(instructions, 5 * FRAME_MS * 1000 * 10);
  player.setCustomMhz(30);
  assert.equal(player.state.limitMhz, 30);
  player.setSpeed("model");
  assert.equal(player.state.limitMhz, 192);
  player.setCustomMhz(20); // sans effet tant que le mode est « modèle »
  assert.equal(player.state.limitMhz, 192);
  player.setSpeed("custom");
  assert.equal(player.state.limitMhz, 20);
  // Une fréquence refusée par le module est ignorée.
  player.setCustomMhz(0);
  assert.equal(player.state.customMhz, 20);
  assert.equal(player.state.limitMhz, 20);
});

await check("sans limite, l'application exécute beaucoup plus que la fréquence du modèle", async () => {
  const max = await executed(2, { speed: "max" });
  assert.equal(max.player.state.limitMhz, null);
  const model = await executed(2);
  assert.ok(
    max.instructions > 4 * model.instructions,
    `${max.instructions} instructions sans limite, ${model.instructions} au rythme du modèle`,
  );
});

await check("à fréquence limitée, le temps de l'application reste le temps réel", async () => {
  const { player } = await executed(30, { speed: "custom", customMhz: 10 });
  const appMs = player.call({ cmd: "time" }).time_ms;
  assert.ok(Math.abs(appMs - 30 * FRAME_MS) < 2, `${appMs} ms pour ${30 * FRAME_MS} ms réelles`);
});

if (failures) {
  console.log(`${failures} échec(s)`);
  process.exit(1);
}
