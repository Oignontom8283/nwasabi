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
