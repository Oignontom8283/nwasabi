// Le mapping des touches (@nwasabi/js) : lecture des fichiers NMK, clavier, gestes tactiles.
// Sans module WebAssembly ni corpus : tout se joue avec une calculatrice factice et une horloge simulée.
//
//   node tests/wasm/keymap.mjs

import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_NMK,
  GestureRecognizer,
  InputMapper,
  KEYS,
  KeyMap,
  NmkError,
} from "../../packages/js/dist/index.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
let failures = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL ${name}\n     ${error.stack ?? error}`);
  }
};

// --- les fichiers partagés avec le cœur Rust (tests/nmk/) ---------------------------------------
const fixtures = join(root, "tests", "nmk");
for (const file of readdirSync(fixtures).filter((f) => f.endsWith(".nmk")).sort()) {
  check(`fichier ${file}`, () => {
    const text = readFileSync(join(fixtures, file), "utf8");
    const expected = JSON.parse(readFileSync(join(fixtures, file.replace(/\.nmk$/, ".expected.json")), "utf8"));
    let actual;
    try {
      actual = JSON.parse(JSON.stringify(KeyMap.parse(text)));
    } catch (error) {
      assert.ok(error instanceof NmkError, String(error));
      actual = { error_path: error.path };
    }
    assert.deepEqual(actual, expected);
  });
}

check("le mapping par défaut est celui de src/keymap/default.nmk", () => {
  const file = JSON.parse(readFileSync(join(root, "src", "keymap", "default.nmk"), "utf8"));
  assert.deepEqual(DEFAULT_NMK, file);
});

check("les noms de touches de la bibliothèque sont ceux du cœur", () => {
  assert.equal(new Set(KEYS).size, KEYS.length);
  assert.equal(KEYS.length, 46);
});

// --- une calculatrice factice et une horloge simulée ----------------------------------------------
function rig(map = KeyMap.default()) {
  const log = [];
  const down = new Set();
  const sink = {
    key(name, isDown) {
      log.push(`${isDown ? "+" : "-"}${name}`);
      if (isDown) down.add(name);
      else down.delete(name);
    },
  };
  let time = 0;
  let timers = [];
  const clock = {
    now: () => time,
    setTimeout: (callback, ms) => {
      const timer = { at: time + ms, callback };
      timers.push(timer);
      return timer;
    },
    clearTimeout: (timer) => {
      timers = timers.filter((t) => t !== timer);
    },
  };
  const advance = (ms) => {
    const end = time + ms;
    for (;;) {
      const next = timers.filter((t) => t.at <= end).sort((a, b) => a.at - b.at)[0];
      if (!next) break;
      timers = timers.filter((t) => t !== next);
      time = next.at;
      next.callback();
    }
    time = end;
  };
  const input = new InputMapper(sink, map, clock);
  const touch = (id, kind, x, y) =>
    input[{ down: "pointerDown", move: "pointerMove", up: "pointerUp" }[kind]]({ pointerId: id, clientX: x, clientY: y });
  return { input, log, down, advance, touch };
}

// --- clavier -------------------------------------------------------------------------------------
check("une touche du clavier appuie et relâche la touche de la calculatrice", () => {
  const { input, log } = rig();
  assert.equal(input.hostKey({ code: "Enter", key: "Enter" }, true), true);
  assert.equal(input.hostKey({ code: "Enter", key: "Enter" }, false), true);
  assert.deepEqual(log, ["+ok", "-ok"]);
});

check("une touche inconnue n'est pas prise en charge", () => {
  const { input, log } = rig();
  assert.equal(input.hostKey({ code: "KeyZ", key: "z" }, true), false);
  assert.equal(input.hostKey({ code: "KeyZ", key: "z" }, false), false);
  assert.deepEqual(log, []);
});

check("la répétition automatique d'une touche n'appuie qu'une fois", () => {
  const { input, log } = rig();
  for (let i = 0; i < 5; i++) input.hostKey({ code: "ArrowUp", key: "ArrowUp" }, true);
  input.hostKey({ code: "ArrowUp", key: "ArrowUp" }, false);
  assert.deepEqual(log, ["+up", "-up"]);
});

const wasd = KeyMap.parse(readFileSync(join(fixtures, "wasd.nmk"), "utf8"));

check("la position prime sur le caractère, et AZERTY comme QWERTY donnent la même touche", () => {
  const { input, log } = rig(wasd);
  input.hostKey({ code: "KeyW", key: "z" }, true); // AZERTY : la touche de la position W produit « z »
  input.hostKey({ code: "KeyW", key: "w" }, true);
  assert.deepEqual(log, ["+up"]);
});

check("une combinaison appuie sur toutes ses touches et les relâche ensemble", () => {
  const { input, log } = rig(wasd);
  input.hostKey({ code: "KeyQ", key: "q" }, true);
  input.hostKey({ code: "KeyQ", key: "q" }, false);
  assert.deepEqual(log, ["+shift", "+alpha", "-shift", "-alpha"]);
});

check("deux entrées sur la même touche : elle reste enfoncée tant qu'une est tenue", () => {
  const map = KeyMap.parse({ nmk: 1, keyboard: { KeyA: "ok", KeyB: ["ok", "back"] } });
  const { input, log, down } = rig(map);
  input.hostKey({ code: "KeyA", key: "a" }, true);
  input.hostKey({ code: "KeyB", key: "b" }, true);
  input.hostKey({ code: "KeyA", key: "a" }, false);
  assert.ok(down.has("ok") && down.has("back"));
  input.hostKey({ code: "KeyB", key: "b" }, false);
  assert.deepEqual(log, ["+ok", "+back", "-ok", "-back"]);
});

check("changer de mapping relâche ce qui était tenu, et le relâchement suit l'ancien mapping", () => {
  const { input, log, down } = rig(wasd);
  input.hostKey({ code: "KeyW", key: "w" }, true);
  input.setKeyMap(KeyMap.default());
  assert.equal(down.size, 0);
  assert.equal(input.hostKey({ code: "KeyW", key: "w" }, false), false);
  assert.deepEqual(log, ["+up", "-up"]);
});

// --- gestes tactiles -----------------------------------------------------------------------------
check("un balayage à un doigt vers la droite appuie brièvement sur la flèche droite (défaut)", () => {
  const { log, advance, touch } = rig();
  touch(1, "down", 100, 100);
  touch(1, "move", 160, 105);
  touch(1, "up", 180, 105);
  assert.deepEqual(log, ["+right"]);
  advance(79);
  assert.deepEqual(log, ["+right"]);
  advance(2);
  assert.deepEqual(log, ["+right", "-right"]);
});

check("les quatre balayages par défaut", () => {
  for (const [dx, dy, key] of [[-80, 0, "left"], [80, 0, "right"], [0, -80, "up"], [0, 80, "down"]]) {
    const { log, touch } = rig();
    touch(1, "down", 200, 200);
    touch(1, "up", 200 + dx, 200 + dy);
    assert.deepEqual(log, [`+${key}`]);
  }
});

check("un déplacement trop court n'est pas un balayage", () => {
  const { log, touch } = rig();
  touch(1, "down", 100, 100);
  touch(1, "up", 110, 100);
  assert.deepEqual(log, []); // pas de règle « tap » par défaut
});

const game = KeyMap.parse(readFileSync(join(fixtures, "wasd.nmk"), "utf8"));

check("un appui long tient la touche tant que le doigt reste posé", () => {
  const { log, advance, touch } = rig(game);
  touch(1, "down", 50, 50);
  advance(399);
  assert.deepEqual(log, []);
  advance(2);
  assert.deepEqual(log, ["+ok"]);
  advance(1000);
  touch(1, "up", 50, 50);
  assert.deepEqual(log, ["+ok", "-ok"]);
});

check("un appui court n'est pas un appui long", () => {
  const { log, advance, touch } = rig(game);
  touch(1, "down", 50, 50);
  advance(100);
  touch(1, "up", 50, 50);
  advance(1000);
  assert.deepEqual(log, []);
});

check("un appui à deux doigts appuie sur la touche de la règle", () => {
  const { log, advance, touch } = rig(game);
  touch(1, "down", 50, 50);
  touch(2, "down", 90, 50);
  advance(50);
  touch(2, "up", 90, 50);
  touch(1, "up", 50, 50);
  assert.deepEqual(log, ["+back"]);
  advance(100);
  assert.deepEqual(log, ["+back", "-back"]);
});

check("un balayage à deux doigts n'a pas de règle : rien ne se passe", () => {
  const { log, touch } = rig(game);
  touch(1, "down", 50, 50);
  touch(2, "down", 90, 50);
  touch(1, "up", 150, 50);
  touch(2, "up", 190, 50);
  assert.deepEqual(log, []);
});

check("un balayage remplacé par le fichier appuie sur une combinaison", () => {
  const { log, touch } = rig(game);
  touch(1, "down", 200, 50);
  touch(1, "up", 100, 50);
  assert.deepEqual(log, ["+shift", "+left"]);
});

check("un balayage retiré par le fichier ne fait rien", () => {
  const { log, touch } = rig(game);
  touch(1, "down", 50, 200);
  touch(1, "up", 50, 100);
  assert.deepEqual(log, []);
});

check("double appui : un seul des deux gestes, l'appui simple attend le délai", () => {
  const map = KeyMap.parse({
    nmk: 1,
    touch: { rules: [{ gesture: "tap", key: "ok" }, { gesture: "double_tap", key: "back" }] },
  });
  const single = rig(map);
  single.touch(1, "down", 5, 5);
  single.touch(1, "up", 5, 5);
  assert.deepEqual(single.log, []);
  single.advance(301);
  assert.deepEqual(single.log, ["+ok"]);

  const double = rig(map);
  double.touch(1, "down", 5, 5);
  double.touch(1, "up", 5, 5);
  double.advance(100);
  double.touch(2, "down", 5, 5);
  double.touch(2, "up", 5, 5);
  double.advance(1000);
  assert.deepEqual(double.log, ["+back", "-back"]);
});

check("un appui long sans « while_held » est ponctuel", () => {
  const map = KeyMap.parse({ nmk: 1, touch: { rules: [{ gesture: "long_press", key: "home" }] } });
  const { log, advance, touch } = rig(map);
  touch(1, "down", 5, 5);
  advance(501);
  assert.deepEqual(log, ["+home"]);
  advance(81);
  touch(1, "up", 5, 5);
  assert.deepEqual(log, ["+home", "-home"]);
});

check("annuler un geste en cours relâche l'appui long", () => {
  const { input, log, advance, touch } = rig(game);
  touch(1, "down", 50, 50);
  advance(500);
  input.pointerCancel();
  assert.deepEqual(log, ["+ok", "-ok"]);
});

if (failures) {
  console.log(`${failures} échec(s)`);
  process.exit(1);
}
