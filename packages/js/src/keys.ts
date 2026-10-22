// Les touches de la calculatrice (noms de l'API EADK, sans le préfixe `eadk_key_`).

export const KEYS = [
  "left", "up", "down", "right", "ok", "back", "home", "on_off", "shift", "alpha", "xnt", "var",
  "toolbox", "backspace", "exp", "ln", "log", "imaginary", "comma", "power", "sine", "cosine",
  "tangent", "pi", "sqrt", "square", "seven", "eight", "nine", "left_parenthesis",
  "right_parenthesis", "four", "five", "six", "multiplication", "division", "one", "two", "three",
  "plus", "minus", "zero", "dot", "ee", "ans", "exe",
] as const;

/** Nom d'une touche de la calculatrice. */
export type KeyName = (typeof KEYS)[number];

/** Touche du clavier de l'ordinateur (`KeyboardEvent.key`) → touche de la calculatrice. */
export const PHYSICAL_KEYS: Readonly<Record<string, KeyName>> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "ok",
  " ": "ok",
  Backspace: "backspace",
  Escape: "back",
  Home: "home",
  Shift: "shift",
  Control: "alpha",
  "0": "zero",
  "1": "one",
  "2": "two",
  "3": "three",
  "4": "four",
  "5": "five",
  "6": "six",
  "7": "seven",
  "8": "eight",
  "9": "nine",
  "+": "plus",
  "-": "minus",
  "*": "multiplication",
  "/": "division",
  ".": "dot",
  ",": "comma",
  "(": "left_parenthesis",
  ")": "right_parenthesis",
  e: "exe",
};
