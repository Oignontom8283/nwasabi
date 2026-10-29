// Mapping des entrées (clavier de l'ordinateur, gestes tactiles) vers les touches de la calculatrice.
// Le format (fichiers `.nmk`) est décrit dans `docs/reference/formats/nmk.md` ; la lecture reproduit celle du cœur Rust
// (`src/keymap/`) et les deux sont testés avec les mêmes fichiers (`tests/nmk/`).

import { KEYS } from "./keys.js";
import type { KeyName } from "./keys.js";

/** Version du format que cette bibliothèque sait lire. */
export const NMK_VERSION = 1;

export const GESTURES = [
  "tap",
  "double_tap",
  "long_press",
  "swipe_left",
  "swipe_right",
  "swipe_up",
  "swipe_down",
] as const;
export type GestureName = (typeof GESTURES)[number];

/** Une règle tactile : ce geste, avec ce nombre de doigts, appuie sur ces touches. */
export interface TouchRule {
  gesture: GestureName;
  fingers: number;
  keys: KeyName[];
  /** Appui long seulement : la touche reste enfoncée tant que le doigt reste posé. */
  whileHeld: boolean;
}

export interface TouchSettings {
  /** Distance minimale d'un balayage, en pixels. */
  swipeDistance: number;
  longPressMs: number;
  doubleTapMs: number;
  /** Durée pendant laquelle la touche reste enfoncée pour un geste ponctuel. */
  pressMs: number;
}

/** Fichier NMK invalide : `path` situe l'erreur (`keyboard.KeyW`, `touch.rules[1].gesture`). */
export class NmkError extends Error {
  constructor(
    readonly path: string,
    message: string,
  ) {
    super(path ? `${path} : ${message}` : message);
    this.name = "NmkError";
  }
}

type Obj = Record<string, unknown>;
type Chord = KeyName[] | null;
interface RawRule {
  gesture: GestureName;
  fingers: number;
  keys: Chord;
  whileHeld: boolean;
}

const isObject = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => typeof v === "number" && Number.isInteger(v) && v >= 0;

function allowOnly(object: Obj, path: string, allowed: string[]): void {
  for (const field of Object.keys(object)) {
    if (!allowed.includes(field)) {
      throw new NmkError(
        path ? `${path}.${field}` : field,
        `champ inconnu (attendus : ${allowed.join(", ")})`,
      );
    }
  }
}

/** Valide une touche de l'ordinateur : `KeyW` (code) ou `key:e` (caractère). */
function checkHostKey(id: string, path: string): void {
  const colon = id.indexOf(":");
  if (colon < 0) {
    if (!/^[A-Za-z0-9]+$/.test(id)) {
      throw new NmkError(path, "un code de touche s'écrit avec des lettres et des chiffres (KeyW, Digit1…)");
    }
    return;
  }
  const prefix = id.slice(0, colon);
  const value = id.slice(colon + 1);
  if (prefix !== "key") {
    throw new NmkError(path, `préfixe inconnu « ${prefix}: » (seul « key: » existe)`);
  }
  if (value === "" || [...value].length > 32) {
    throw new NmkError(path, "valeur vide ou trop longue après « key: »");
  }
}

function parseChord(value: unknown, path: string): Chord {
  const one = (v: unknown): KeyName => {
    if (typeof v !== "string") throw new NmkError(path, "un nom de touche est attendu");
    if (!(KEYS as readonly string[]).includes(v)) {
      throw new NmkError(path, `touche de la calculatrice inconnue « ${v} »`);
    }
    return v as KeyName;
  };
  if (value === null) return null;
  if (Array.isArray(value)) {
    if (value.length < 1 || value.length > 4) {
      throw new NmkError(path, "une combinaison compte de 1 à 4 touches");
    }
    const chord: KeyName[] = [];
    for (const item of value) {
      const key = one(item);
      if (chord.includes(key)) throw new NmkError(path, `touche « ${key} » répétée`);
      chord.push(key);
    }
    return chord;
  }
  return [one(value)];
}

function parseRule(value: unknown, path: string): RawRule {
  if (!isObject(value)) throw new NmkError(path, "un objet est attendu");
  allowOnly(value, path, ["gesture", "fingers", "key", "while_held"]);
  const name = value.gesture;
  if (typeof name !== "string") throw new NmkError(`${path}.gesture`, "geste obligatoire");
  if (!(GESTURES as readonly string[]).includes(name)) {
    throw new NmkError(`${path}.gesture`, `geste inconnu « ${name} »`);
  }
  const gesture = name as GestureName;
  let fingers = 1;
  if ("fingers" in value) {
    const n = value.fingers;
    if (typeof n !== "number" || !Number.isInteger(n) || n < 1 || n > 3) {
      throw new NmkError(`${path}.fingers`, "1, 2 ou 3 est attendu");
    }
    fingers = n;
  }
  if (!("key" in value)) {
    throw new NmkError(`${path}.key`, "touche obligatoire (null pour retirer la règle par défaut)");
  }
  const keys = parseChord(value.key, `${path}.key`);
  let whileHeld = false;
  if ("while_held" in value) {
    if (typeof value.while_held !== "boolean") {
      throw new NmkError(`${path}.while_held`, "vrai ou faux est attendu");
    }
    if (value.while_held && gesture !== "long_press") {
      throw new NmkError(`${path}.while_held`, "réservé à long_press");
    }
    whileHeld = value.while_held;
  }
  return { gesture, fingers, keys, whileHeld };
}

const SETTINGS: [string, keyof TouchSettings, number, number][] = [
  ["swipe_distance", "swipeDistance", 5, 1000],
  ["long_press_ms", "longPressMs", 50, 10_000],
  ["double_tap_ms", "doubleTapMs", 50, 5_000],
  ["press_ms", "pressMs", 10, 5_000],
];

/** Un mapping lu, complété par les liaisons par défaut si le fichier le demande. */
export class KeyMap {
  private constructor(
    readonly name: string | null,
    private readonly keyboard: Map<string, KeyName[]>,
    readonly settings: TouchSettings,
    readonly rules: readonly TouchRule[],
  ) {}

  /** Lit un fichier NMK (texte JSON ou valeur déjà analysée) ; lève `NmkError`. */
  static parse(source: string | unknown): KeyMap {
    let root: unknown = source;
    if (typeof source === "string") {
      try {
        root = JSON.parse(source);
      } catch (e) {
        throw new NmkError("", `JSON invalide : ${(e as Error).message}`);
      }
    }
    if (!isObject(root)) throw new NmkError("", "le fichier doit être un objet JSON");
    allowOnly(root, "", ["nmk", "name", "extends", "keyboard", "touch"]);
    if (!("nmk" in root)) throw new NmkError("nmk", 'version du format obligatoire ("nmk": 1)');
    if (root.nmk !== NMK_VERSION) {
      throw new NmkError(
        "nmk",
        `version ${JSON.stringify(root.nmk)} non prise en charge (seule ${NMK_VERSION} l'est)`,
      );
    }
    let name: string | null = null;
    if ("name" in root) {
      if (typeof root.name !== "string" || [...root.name].length > 100) {
        throw new NmkError("name", "une chaîne de 100 caractères au plus est attendue");
      }
      name = root.name;
    }
    let extendsDefault = true;
    if ("extends" in root) {
      if (root.extends === "none") extendsDefault = false;
      else if (root.extends !== "default") throw new NmkError("extends", "« default » ou « none » est attendu");
    }

    const keyboard: [string, Chord][] = [];
    if ("keyboard" in root) {
      if (!isObject(root.keyboard)) throw new NmkError("keyboard", "un objet est attendu");
      for (const [id, value] of Object.entries(root.keyboard)) {
        const path = `keyboard.${id}`;
        checkHostKey(id, path);
        keyboard.push([id, parseChord(value, path)]);
      }
    }

    const given: Partial<TouchSettings> = {};
    const rules: RawRule[] = [];
    if ("touch" in root) {
      const touch = root.touch;
      if (!isObject(touch)) throw new NmkError("touch", "un objet est attendu");
      allowOnly(touch, "touch", [...SETTINGS.map(([field]) => field), "rules"]);
      for (const [field, property, min, max] of SETTINGS) {
        if (!(field in touch)) continue;
        const n = touch[field];
        if (!isCount(n) || n < min || n > max) {
          throw new NmkError(`touch.${field}`, `un entier de ${min} à ${max} est attendu`);
        }
        given[property] = n;
      }
      if ("rules" in touch) {
        if (!Array.isArray(touch.rules)) throw new NmkError("touch.rules", "une liste est attendue");
        touch.rules.forEach((value, i) => {
          const rule = parseRule(value, `touch.rules[${i}]`);
          if (rules.some((r) => r.gesture === rule.gesture && r.fingers === rule.fingers)) {
            throw new NmkError(
              `touch.rules[${i}]`,
              `règle en double pour ${rule.gesture} avec ${rule.fingers} doigt(s)`,
            );
          }
          rules.push(rule);
        });
      }
    }

    const base = extendsDefault ? KeyMap.default() : null;
    const merged = new Map(base?.keyboard);
    for (const [id, chord] of keyboard) {
      if (chord) merged.set(id, chord);
      else merged.delete(id);
    }
    const settings: TouchSettings = { ...(base?.settings ?? DEFAULT_SETTINGS), ...given };
    let all = [...(base?.rules ?? [])];
    for (const rule of rules) {
      all = all.filter((r) => !(r.gesture === rule.gesture && r.fingers === rule.fingers));
      if (rule.keys) all.push({ ...rule, keys: rule.keys });
    }
    return new KeyMap(name, merged, settings, all);
  }

  private static defaultMap: KeyMap | null = null;

  /** Le mapping par défaut. */
  static default(): KeyMap {
    if (!KeyMap.defaultMap) {
      const source = { ...DEFAULT_NMK, extends: "none" };
      KeyMap.defaultMap = KeyMap.parse(source);
    }
    return KeyMap.defaultMap;
  }

  /** Touches de la calculatrice pour une touche de l'ordinateur ; la position (`code`) prime sur le caractère (`key`). */
  resolve(code: string, key: string): readonly KeyName[] | undefined {
    return (code ? this.keyboard.get(code) : undefined) ?? this.keyboard.get(`key:${key}`);
  }

  // ---- modification (les mappings sont immuables : chaque changement en rend un nouveau) ----------------

  private derive(patch: {
    name?: string | null;
    keyboard?: Map<string, KeyName[]>;
    settings?: Partial<TouchSettings>;
    rules?: readonly TouchRule[];
  }): KeyMap {
    return new KeyMap(
      patch.name === undefined ? this.name : patch.name,
      patch.keyboard ?? new Map(this.keyboard),
      { ...this.settings, ...patch.settings },
      patch.rules ?? this.rules,
    );
  }

  /** Donne un nom au mapping (`null` : sans nom). */
  withName(name: string | null): KeyMap {
    return this.derive({ name });
  }

  /** Lie une touche de l'ordinateur (`KeyW`, `key:e`) à des touches de la calculatrice ; `null` retire la liaison. */
  withBinding(host: string, chord: readonly KeyName[] | null): KeyMap {
    checkHostKey(host, host);
    const keyboard = new Map(this.keyboard);
    if (chord === null || chord.length === 0) keyboard.delete(host);
    else keyboard.set(host, parseChord([...chord], host) as KeyName[]);
    return this.derive({ keyboard });
  }

  /** Remplace (ou ajoute) la règle de ce geste avec ce nombre de doigts ; `keys` à `null` la retire. */
  withRule(rule: { gesture: GestureName; fingers?: number; keys: readonly KeyName[] | null; whileHeld?: boolean }): KeyMap {
    const fingers = rule.fingers ?? 1;
    const rules = this.rules.filter((r) => !(r.gesture === rule.gesture && r.fingers === fingers));
    if (rule.keys && rule.keys.length > 0) {
      const whileHeld = rule.whileHeld ?? false;
      parseRule(
        { gesture: rule.gesture, fingers, key: [...rule.keys], while_held: whileHeld },
        `rule ${rule.gesture}`,
      );
      rules.push({ gesture: rule.gesture, fingers, keys: [...rule.keys], whileHeld });
    }
    return this.derive({ rules });
  }

  /** Change des réglages tactiles (distances et durées), avec les mêmes limites que dans un fichier. */
  withSettings(patch: Partial<TouchSettings>): KeyMap {
    for (const [field, property, min, max] of SETTINGS) {
      const n = patch[property];
      if (n !== undefined && (!isCount(n) || n < min || n > max)) {
        throw new NmkError(`touch.${field}`, `un entier de ${min} à ${max} est attendu`);
      }
    }
    return this.derive({ settings: patch });
  }

  /** Ce qui appuie sur cette touche de la calculatrice : touches de l'ordinateur et gestes. */
  sourcesOf(key: KeyName): { keyboard: string[]; rules: TouchRule[] } {
    return {
      keyboard: this.entries()
        .filter(([, chord]) => chord.includes(key))
        .map(([id]) => id),
      rules: this.rules.filter((r) => r.keys.includes(key)),
    };
  }

  /**
   * Le mapping sous la forme d'un fichier NMK, autonome (`"extends": "none"`) : il se recharge à l'identique
   * quel que soit le mapping par défaut de la version de nwasabi qui le lit.
   */
  toNmk(): object {
    const keyboard: Record<string, string | string[]> = {};
    for (const [id, chord] of this.entries()) keyboard[id] = chord.length === 1 ? chord[0]! : [...chord];
    return {
      nmk: NMK_VERSION,
      ...(this.name ? { name: this.name } : {}),
      extends: "none",
      keyboard,
      touch: {
        swipe_distance: this.settings.swipeDistance,
        long_press_ms: this.settings.longPressMs,
        double_tap_ms: this.settings.doubleTapMs,
        press_ms: this.settings.pressMs,
        rules: this.rules.map((r) => ({
          gesture: r.gesture,
          fingers: r.fingers,
          key: r.keys.length === 1 ? r.keys[0] : [...r.keys],
          ...(r.whileHeld ? { while_held: true } : {}),
        })),
      },
    };
  }

  /** Les liaisons du clavier, triées par identifiant. */
  entries(): [string, readonly KeyName[]][] {
    return [...this.keyboard.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  }

  /** Le mapping complet, sous la forme que produit `nwasabi-cli keys --nmk --json` (sert aux tests). */
  toJSON(): unknown {
    const keyboard: Record<string, string[]> = {};
    for (const [id, chord] of this.entries()) keyboard[id] = [...chord];
    return {
      name: this.name,
      keyboard,
      touch: {
        swipe_distance: this.settings.swipeDistance,
        long_press_ms: this.settings.longPressMs,
        double_tap_ms: this.settings.doubleTapMs,
        press_ms: this.settings.pressMs,
        rules: this.rules.map((r) => ({
          gesture: r.gesture,
          fingers: r.fingers,
          key: [...r.keys],
          while_held: r.whileHeld,
        })),
      },
    };
  }
}

const DEFAULT_SETTINGS: TouchSettings = {
  swipeDistance: 40,
  longPressMs: 500,
  doubleTapMs: 300,
  pressMs: 80,
};

// Copie de `src/keymap/default.nmk` (un test vérifie qu'elles sont identiques).
export const DEFAULT_NMK = {
  "nmk": 1,
  "name": "Default",
  "extends": "none",
  "keyboard": {
    "key:ArrowUp": "up",
    "key:ArrowDown": "down",
    "key:ArrowLeft": "left",
    "key:ArrowRight": "right",
    "key:Enter": "ok",
    "key: ": "ok",
    "key:Backspace": "backspace",
    "key:Escape": "back",
    "key:Home": "home",
    "key:Shift": "shift",
    "key:Control": "alpha",
    "key:0": "zero",
    "key:1": "one",
    "key:2": "two",
    "key:3": "three",
    "key:4": "four",
    "key:5": "five",
    "key:6": "six",
    "key:7": "seven",
    "key:8": "eight",
    "key:9": "nine",
    "key:+": "plus",
    "key:-": "minus",
    "key:*": "multiplication",
    "key:/": "division",
    "key:.": "dot",
    "key:,": "comma",
    "key:(": "left_parenthesis",
    "key:)": "right_parenthesis",
    "key:e": "exe"
  },
  "touch": {
    "rules": [
      {
        "gesture": "swipe_left",
        "key": "left"
      },
      {
        "gesture": "swipe_right",
        "key": "right"
      },
      {
        "gesture": "swipe_up",
        "key": "up"
      },
      {
        "gesture": "swipe_down",
        "key": "down"
      }
    ]
  }
};
