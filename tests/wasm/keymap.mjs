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

check("un déplacement trop court n'est pas un balayage mais un appui, qui appuie sur OK (défaut)", () => {
  const { log, touch } = rig();
  touch(1, "down", 100, 100);
  touch(1, "up", 110, 100);
  assert.deepEqual(log, ["+ok"]);
});

check("un clic de souris n'appuie sur rien, mais un balayage à la souris oui", () => {
  const { input, log } = rig();
  const mouse = (kind, x, y) => input[{ down: "pointerDown", up: "pointerUp" }[kind]]({ pointerId: 7, pointerType: "mouse", clientX: x, clientY: y });
  mouse("down", 100, 100);
  mouse("up", 100, 100);
  assert.deepEqual(log, []);
  mouse("down", 100, 100);
  mouse("up", 180, 100);
  assert.deepEqual(log, ["+right"]);
});

check("un doigt qui touche l'écran appuie sur OK", () => {
  const { input, log } = rig();
  input.pointerDown({ pointerId: 1, pointerType: "touch", clientX: 100, clientY: 100 });
  input.pointerUp({ pointerId: 1, pointerType: "touch", clientX: 100, clientY: 100 });
  assert.deepEqual(log, ["+ok"]);
});

// Un demi-cercle (ou un demi-tour) : le doigt part, tourne et revient en sens inverse.
const arc = (cx, cy, r, from, to, steps = 18) =>
  Array.from({ length: steps + 1 }, (_, i) => {
    const a = ((from + ((to - from) * i) / steps) * Math.PI) / 180;
    return [Math.round(cx + r * Math.cos(a)), Math.round(cy + r * Math.sin(a))];
  });
const drag = (touch, points) => {
  touch(1, "down", ...points[0]);
  for (const point of points.slice(1, -1)) touch(1, "move", ...point);
  touch(1, "up", ...points[points.length - 1]);
};

check("un demi-cercle appuie sur retour (défaut), dans les deux sens et dans toutes les orientations", () => {
  for (const [from, to] of [[-90, 90], [90, -90], [180, 0], [0, -180], [-60, 130], [20, 215]]) {
    const { log, touch } = rig();
    drag(touch, arc(200, 200, 40, from, to));
    assert.deepEqual(log, ["+back"], `de ${from}° à ${to}°`);
  }
});

check("un quart de cercle n'est pas un demi-tour : c'est un balayage", () => {
  const { log, touch } = rig();
  drag(touch, arc(200, 200, 60, -90, 0));
  assert.deepEqual(log, ["+right"]);
});

check("un balayage droit n'est pas un demi-tour", () => {
  const { log, touch } = rig();
  drag(touch, [[100, 100], [130, 100], [160, 100], [190, 100], [220, 100]]);
  assert.deepEqual(log, ["+right"]);
});

check("un zigzag n'est pas un demi-tour : seul son déplacement d'ensemble compte (un balayage)", () => {
  const { log, touch } = rig();
  drag(touch, [[100, 100], [140, 120], [100, 140], [140, 160], [100, 180], [140, 200]]);
  assert.deepEqual(log, ["+down"]);
});

check("un cercle entier n'est pas un demi-tour", () => {
  const { log, touch } = rig();
  drag(touch, arc(200, 200, 40, 0, 360, 36));
  assert.deepEqual(log, ["+ok"]);
});

check("un petit tremblement du doigt reste un appui", () => {
  const { log, touch } = rig();
  drag(touch, [[100, 100], [103, 99], [101, 104], [99, 101], [102, 100]]);
  assert.deepEqual(log, ["+ok"]);
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

check("un appui court n'est pas un appui long : c'est un appui, bref", () => {
  const { log, advance, touch } = rig(game);
  touch(1, "down", 50, 50);
  advance(100);
  touch(1, "up", 50, 50);
  assert.deepEqual(log, ["+ok"]);
  advance(1000);
  assert.deepEqual(log, ["+ok", "-ok"]);
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

// --- modification et export ----------------------------------------------------------------------
check("withBinding, withRule et withName rendent de nouveaux mappings sans toucher l'ancien", () => {
  const base = KeyMap.default();
  const edited = base.withBinding("KeyW", ["up"]).withRule({ gesture: "double_tap", keys: ["ok"] }).withName("édité");
  assert.deepEqual(edited.resolve("KeyW", "w"), ["up"]);
  assert.equal(base.resolve("KeyW", "w"), undefined);
  assert.equal(edited.name, "édité");
  assert.ok(edited.rules.some((r) => r.gesture === "double_tap"));
  assert.ok(!base.rules.some((r) => r.gesture === "double_tap"));
});

check("retirer une liaison ou une règle", () => {
  const edited = KeyMap.default().withBinding("key:e", null).withRule({ gesture: "swipe_up", keys: null });
  assert.equal(edited.resolve("", "e"), undefined);
  assert.ok(!edited.rules.some((r) => r.gesture === "swipe_up"));
});

check("les modifications refusent ce qu'un fichier refuserait", () => {
  assert.throws(() => KeyMap.default().withBinding("char:w", ["up"]), NmkError);
  assert.throws(() => KeyMap.default().withBinding("KeyW", ["haut"]), NmkError);
  assert.throws(() => KeyMap.default().withSettings({ pressMs: 1 }), NmkError);
  assert.throws(() => KeyMap.default().withRule({ gesture: "tap", keys: ["ok"], whileHeld: true }), NmkError);
});

check("toNmk donne un fichier autonome qui se relit à l'identique", () => {
  for (const map of [KeyMap.default(), wasd, KeyMap.default().withBinding("KeyZ", ["ok", "back"])]) {
    const file = JSON.parse(JSON.stringify(map.toNmk()));
    assert.equal(file.extends, "none");
    const again = KeyMap.parse(file);
    assert.deepEqual(JSON.parse(JSON.stringify(again)), JSON.parse(JSON.stringify(map)));
  }
});

check("sourcesOf donne les touches de l'ordinateur et les gestes d'une touche de la calculatrice", () => {
  const map = wasd;
  const up = map.sourcesOf("up");
  assert.ok(up.keyboard.includes("KeyW") && up.keyboard.includes("key:ArrowUp"));
  assert.deepEqual(map.sourcesOf("xnt"), { keyboard: [], rules: [] });
  assert.ok(map.sourcesOf("back").rules.some((r) => r.gesture === "tap" && r.fingers === 2));
});

if (failures) {
  console.log(`${failures} échec(s)`);
  process.exit(1);
}
