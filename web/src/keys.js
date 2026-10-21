// Disposition des touches de la calculatrice (noms de l'API EADK) et correspondance clavier.

/** Lignes du clavier à l'écran : [nom EADK, libellé]. */
export const KEY_ROWS = [
  [["back", "⌫ back"], ["home", "home"], ["on_off", "on/off"]],
  [["shift", "shift"], ["alpha", "alpha"], ["xnt", "x,n,t"], ["var", "var"], ["toolbox", "toolbox"], ["backspace", "⌦"]],
  [["exp", "exp"], ["ln", "ln"], ["log", "log"], ["imaginary", "i"], ["comma", ","], ["power", "x^y"]],
  [["sine", "sin"], ["cosine", "cos"], ["tangent", "tan"], ["pi", "π"], ["sqrt", "√"], ["square", "x²"]],
  [["seven", "7"], ["eight", "8"], ["nine", "9"], ["left_parenthesis", "("], ["right_parenthesis", ")"]],
  [["four", "4"], ["five", "5"], ["six", "6"], ["multiplication", "×"], ["division", "÷"]],
  [["one", "1"], ["two", "2"], ["three", "3"], ["plus", "+"], ["minus", "−"]],
  [["zero", "0"], ["dot", "."], ["ee", "EE"], ["ans", "Ans"], ["exe", "EXE"]],
];

/** Touches directionnelles et OK, dessinées à part. */
export const PAD = { up: "▲", left: "◀", right: "▶", down: "▼", ok: "OK" };

/** Touche du clavier de l'ordinateur → touche EADK. */
export const PHYSICAL = {
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
  0: "zero",
  1: "one",
  2: "two",
  3: "three",
  4: "four",
  5: "five",
  6: "six",
  7: "seven",
  8: "eight",
  9: "nine",
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
