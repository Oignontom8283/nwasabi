// L'horloge du lecteur : l'application ne voit passer que le temps réellement écoulé pendant que la
// simulation tourne. Une pause, ou un onglet caché, arrête le temps pour elle.
//
//   node tests/wasm/player.mjs <dossier de .nwa>
//
// Le temps et `requestAnimationFrame` sont simulés : le test est instantané et déterministe. Sans
// dossier (ou sans NWASABI_CORPUS), le test est ignoré.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Player } from "../../packages/js/dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const wasmPath =
  process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm");
const corpus = process.argv[2] ?? process.env.NWASABI_CORPUS;
if (!corpus) {
  console.log("corpus absent : test de l'horloge ignoré (donner un dossier ou NWASABI_CORPUS)");
  process.exit(0);
}

// --- temps et animation simulés -----------------------------------------------------------------
let now = 0;
let callbacks = [];
Object.defineProperty(globalThis, "performance", { value: { now: () => now }, configurable: true });
globalThis.requestAnimationFrame = (callback) => callbacks.push(callback);
globalThis.cancelAnimationFrame = () => {
  callbacks = [];
};
/** Fait passer `ms` millisecondes réelles, une image du navigateur tous les `step` ms. */
function wait(ms, step = 1000 / 60) {
  for (let t = 0; t < ms; t += step) {
    now += step;
    const due = callbacks;
    callbacks = [];
    for (const callback of due) callback(now);
  }
}

const file = readdirSync(corpus).find((f) => f.startsWith("compressor-1.0.0"));
assert.ok(file, "compressor-1.0.0.nwa dans le corpus");
const nwa = new Uint8Array(readFileSync(join(corpus, file)));

const player = await Player.create(readFileSync(wasmPath));
/** Temps vu par l'application (ms), sans rien lui faire exécuter de plus qu'une instruction. */
const appTime = () => player.call({ cmd: "run", instructions: 1 }).time_ms;
/** Nombre de lectures du clavier faites par l'application. */
const scans = () => player.call({ cmd: "svc_stats" }).counts.find((c) => c.svc === 34)?.calls ?? 0;

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

assert.ok(player.load(nwa, "compressor"));
wait(1000);
const t1 = appTime();
check("le temps de l'application suit le temps réel (1 s ≈ 1000 ms)", () => {
  assert.ok(Math.abs(t1 - 1000) <= 50, `temps vu : ${t1} ms`);
});

check("une pause n'est pas vue par l'application", () => {
  player.stop();
  wait(10_000); // dix secondes réelles pendant lesquelles rien ne tourne
  const during = appTime();
  assert.equal(during, t1, "le temps ne doit pas avancer pendant la pause");
  player.start();
  wait(1000);
  const t2 = appTime();
  assert.ok(Math.abs(t2 - t1 - 1000) <= 100, `après la pause : +${t2 - t1} ms au lieu de ~1000`);
});

check("un onglet caché (aucune image pendant 5 s) vaut au plus 250 ms", () => {
  const before = appTime();
  now += 5000; // aucune image du navigateur pendant ce temps
  wait(1000 / 60); // la première image qui revient
  const after = appTime();
  assert.ok(after - before <= 250 + 20, `+${after - before} ms`);
});

check("msleep(10) dure vraiment ~10 ms réelles (≈ 100 lectures du clavier par seconde)", () => {
  const before = scans();
  wait(1000);
  const n = scans() - before;
  assert.ok(n >= 85 && n <= 115, `${n} lectures en une seconde réelle`);
});

check("un écran à 144 Hz garde la cadence de 60 images par seconde", () => {
  const frames = player.state.frames;
  const before = appTime();
  wait(1000, 1000 / 144);
  assert.ok(Math.abs(appTime() - before - 1000) <= 60, "le temps suit toujours le temps réel");
  assert.ok(player.state.frames - frames <= 70, `${player.state.frames - frames} images en 1 s`);
});

process.exit(failures ? 1 : 0);
