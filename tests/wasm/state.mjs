// Enregistrer et reprendre l'état d'une émulation depuis @nwasabi/js (formats .nds et .nfs), sans corpus :
// l'application est tests/apps/bin/keys.nwa.
//
//   node tests/wasm/state.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Player } from "../../packages/js/dist/index.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const wasm = readFileSync(
  process.env.NWASABI_WASM ?? join(root, "target/wasm32-unknown-unknown/wasm/nwasabi.wasm"),
);
const keys = new Uint8Array(readFileSync(join(root, "tests/apps/bin/keys.nwa")));

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

let now = 0;
let callbacks = [];
const scheduler = {
  now: () => now,
  request: (callback) => callbacks.push(callback),
  cancel: () => {
    callbacks = [];
  },
};
const frames = (count) => {
  for (let i = 0; i < count; i++) {
    now += 1000 / 60;
    const due = callbacks;
    callbacks = [];
    for (const callback of due) callback(now);
  }
};
const create = async () => {
  const player = await Player.create(wasm, { scheduler });
  player.load(keys, "keys.nwa");
  return player;
};
const screen = (player) => Array.from(player.framebuffer()).join(",");

await check("l'enregistrement exige la pause", async () => {
  const player = await create();
  frames(3);
  assert.equal(player.dump(), null);
  assert.match(player.state.error, /pause/);
  player.stop();
  assert.equal(player.dump().nds, 1);
});

await check("une reprise redonne le même écran, le même temps, et se poursuit de la même façon", async () => {
  const a = await create();
  frames(5);
  a.key("up", true);
  frames(5);
  a.stop();
  const document = JSON.parse(JSON.stringify(a.dump()));

  const b = await create();
  assert.equal(b.restore(document), true);
  assert.equal(b.state.running, false);
  assert.equal(b.state.appName, a.state.appName);
  assert.equal(screen(b), screen(a));
  assert.equal(b.state.timeMs, a.time());

  // Les deux continuent, la touche reste tenue : même écran à la fin.
  a.start();
  b.start();
  frames(4);
  a.stop();
  b.stop();
  assert.equal(screen(b), screen(a));
});

await check("un document invalide est refusé sans toucher l'appareil", async () => {
  const player = await create();
  frames(3);
  player.stop();
  const before = screen(player);
  assert.equal(player.restore({ nds: 1 }), false);
  assert.match(player.state.error, /device/);
  assert.equal(player.state.loaded, true);
  assert.equal(screen(player), before);
});

await check("sans la partie application, celle de l'appareil actuel est gardée", async () => {
  const a = await create();
  frames(3);
  a.stop();
  const document = a.dump(["memory", "state"]);
  assert.equal(document.app, undefined);
  const b = await create();
  assert.equal(b.restore(document), true);
  assert.equal(screen(b), screen(a));
});

await check("les fichiers de la calculatrice : export puis import (.nfs)", async () => {
  const a = await create();
  a.call({ cmd: "fs_write", name: "notes.py", text: "print(42)" });
  const exported = a.call({ cmd: "fs_export" });
  assert.equal(exported.nfs.nfs, 1);
  const b = await create();
  assert.equal(b.call({ cmd: "fs_import", nfs: exported.nfs }).ok, true);
  assert.deepEqual(
    b.call({ cmd: "fs_list" }).files.map((f) => f.name),
    ["notes.py"],
  );
});

if (failures) {
  console.log(`${failures} échec(s)`);
  process.exit(1);
}
