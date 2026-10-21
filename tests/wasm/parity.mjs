// Parité natif / WebAssembly : la même application, exécutée par le module wasm sous Node.js,
// doit donner exactement les mêmes écrans que la version native (références de tests/golden/).
//
//   node tests/wasm/parity.mjs <dossier de .nwa>
//
// Sans dossier (ou sans la variable NWASABI_CORPUS), le test est ignoré.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { Nwasabi, toBase64, fromBase64 } from "../../web/src/nwasabi.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const wasmPath =
  process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/release/nwasabi.wasm");

const corpus = process.argv[2] ?? process.env.NWASABI_CORPUS;
const emu = await Nwasabi.load(readFileSync(wasmPath));
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

// --- sans corpus : le protocole et les fichiers ---------------------------------------------
check("help liste les commandes", () => {
  const reply = emu.call({ cmd: "help" });
  assert.ok(reply.commands.includes("fs_write"));
});
check("les erreurs sont des réponses", () => {
  const reply = emu.request({ cmd: "fs_list" });
  assert.equal(reply.ok, false);
  assert.match(reply.error, /aucun appareil/);
});
check("fichiers et réglages", () => {
  emu.call({ cmd: "new", model: "n0120" });
  emu.call({ cmd: "fs_write", name: "a.py", text: "print(1)", python: true, auto_import: true });
  assert.equal(emu.call({ cmd: "fs_list" }).files.length, 1);
  assert.equal(emu.call({ cmd: "fs_read", name: "a.py", python: true }).text, "print(1)");
  emu.call({ cmd: "settings_set", name: "device_name", value: "wasm" });
  const st = emu.call({ cmd: "inspect" }).structures;
  assert.equal(st.device_name, "wasm");
  assert.equal(st.sram_base, 0x24000000);
});
check("écran noir au départ", () => {
  const pixels = emu.framebuffer();
  assert.equal(pixels.length, 320 * 240);
  assert.ok(pixels.every((p) => p === 0));
});

// --- avec corpus : mêmes écrans que le natif --------------------------------------------------
if (!corpus) {
  console.log("corpus absent : parité ignorée (donner un dossier ou NWASABI_CORPUS)");
} else {
  const golden = new Map(
    readFileSync(join(root, "tests/golden/screens.tsv"), "utf8")
      .split("\n")
      .filter((l) => l && !l.startsWith("#"))
      .map((l) => l.split("\t"))
      .map(([name, stop, frames, instructions, crc]) => [name, { stop, instructions, crc }]),
  );
  // Applications qui s'exécutent exactement 40 M instructions sans dormir : même état final.
  // (conway et minesweeper récents sont faits pour le N0120 : le test du corpus les y relance.)
  const apps = [
    ["sierpinski", "n0110"],
    ["mandelbrot", "n0110"],
    ["periodic", "n0110"],
    ["conway-1.2.3", "n0120"],
    ["minesweeper-1.3.5", "n0120"],
  ];
  for (const [name, model] of apps) {
    const file = readdirSync(corpus).find((f) => f.startsWith(name));
    if (!file) continue;
    check(`parité écran ${name}`, () => {
      const nwa = readFileSync(join(corpus, file));
      emu.call({ cmd: "load", model, nwa_base64: toBase64(nwa) });
      emu.call({ cmd: "run", instructions: 40_000_000 });
      const crc = emu.call({ cmd: "screen" }).crc32;
      assert.equal(crc, golden.get(name).crc, "empreinte de l'écran différente du natif");
    });
  }
  check("capture PNG", () => {
    const png = fromBase64(emu.call({ cmd: "screen", png: true }).png_base64);
    assert.deepEqual([...png.slice(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  });
}

process.exit(failures ? 1 : 0);
