// @nwasabi/js dans Node.js « classique » : pas de navigateur, pas de canvas, pas de
// `requestAnimationFrame`. Le lecteur se rythme tout seul avec des minuteries et rend l'écran en pixels
// bruts.
//
//   node tests/wasm/node.mjs <dossier de .nwa>
//
// Ce test dure environ deux secondes réelles. Sans dossier (ou NWASABI_CORPUS), il est ignoré.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Player, PHYSICAL_KEYS } from "../../packages/js/dist/index.js";
import { corpusDir } from "./corpus.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath =
  process.env.NWASABI_WASM ??
  join(here, "..", "..", "target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const corpus = corpusDir();
if (!corpus) {
  process.exit(0);
}
for (const name of ["window", "document", "requestAnimationFrame", "ImageData", "HTMLCanvasElement"]) {
  assert.equal(typeof globalThis[name], "undefined", `${name} ne doit pas exister ici`);
}

const file = readdirSync(corpus).find((f) => f.startsWith("minesweeper-1.0.2"));
const nwa = new Uint8Array(readFileSync(join(corpus, file)));

let frames = 0;
let lastFrame = null;
const player = await Player.create(readFileSync(wasmPath), {
  onFrame(frame) {
    frames++;
    lastFrame = { width: frame.width, height: frame.height, pixels: frame.rgb565.length, rgba: frame.rgba(), time: frame.timeMs };
  },
});
assert.ok(player.load(nwa, file), player.state.error);
await new Promise((resolve) => setTimeout(resolve, 1500));
player.stop();

let failures = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failures++;
    console.log(`FAIL ${name}\n     ${e.message}`);
  }
};

check("le lecteur tourne seul à ~60 images par seconde avec les minuteries de Node", () => {
  assert.ok(frames >= 60 && frames <= 110, `${frames} images en 1,5 s`);
});
check("l'écran sort en pixels bruts (RGB565 et RGBA)", () => {
  assert.deepEqual([lastFrame.width, lastFrame.height, lastFrame.pixels], [320, 240, 76800]);
  assert.equal(lastFrame.rgba.length, 320 * 240 * 4);
  assert.ok(lastFrame.rgba.some((v, i) => i % 4 !== 3 && v !== 0), "des pixels sont allumés");
  assert.equal(player.rgba().length, 320 * 240 * 4);
  assert.equal(player.framebuffer().length, 76800);
});
check("le temps de simulation suit le temps réel", () => {
  assert.ok(Math.abs(player.time() - 1500) <= 250, `${player.time()} ms`);
});
const framesAtStop = frames;
await new Promise((resolve) => setTimeout(resolve, 200));
check("après stop(), plus aucune image (la minuterie est annulée)", () => {
  assert.equal(frames, framesAtStop);
});
check("la table des touches ne dépend pas du navigateur", () => {
  assert.equal(PHYSICAL_KEYS.Enter, "ok");
});

process.exitCode = failures ? 1 : 0;
