// Dessin de la calculatrice, aux proportions et aux positions de touches d'une vraie NumWorks : repère de
// 1160 × 2220, écran en 776 × 582 (4:3), touches de 129 × 87 (rangées de six) ou 156 × 101 (rangées de cinq).
// Les dimensions et les positions viennent de la disposition publiée du simulateur d'Omega (des faits de
// géométrie, qu'on ne peut pas changer : ce sont ceux de l'appareil) ; les formes, couleurs et dessins sont les nôtres.

import type { KeyName } from "@nwasabi/js";

// Le repère des touches et de l'écran va de 0 à 1160 × 2220 ; la vue est rognée autour du boîtier pour qu'il y ait peu de
// marge entre le bord et les touches : à taille de page égale, l'écran et les touches sont plus grands.
export const VIEW = { x: 36, y: 14, width: 1088, height: 2180 };
export const BODY = { ...VIEW, radius: 84 };
export const BEZEL = { x: 152, y: 151, width: 856, height: 662, radius: 40 };
/** L'écran : 776 × 582, soit 4:3 comme les 320 × 240 pixels. */
export const SCREEN = { x: 192, y: 191, width: 776, height: 582 };

/** Les deux bras de la croix directionnelle (rectangles arrondis dessinés sous les touches). */
export const CROSS = [
  { x: 133, y: 983, width: 282, height: 87, radius: 43 },
  { x: 231, y: 886, width: 87, height: 280, radius: 43 },
];

const GLYPHS = {
  home: "M3 11l9-8 9 8 M5 10v10h5v-6h4v6h5V10",
  power: "M12 3v9 M6.3 7a8 8 0 1 0 11.4 0",
  back: "M9 14l-4-4 4-4 M5 10h9a5 5 0 0 1 0 10h-3",
};

export interface CalcKey {
  /** Nom EADK de la touche. */
  name: KeyName;
  label: string;
  /** Légendes au-dessus : seconde fonction (shift) et lettre (alpha). */
  shift?: string;
  alpha?: string;
  style: "plain" | "mod" | "small" | "nav" | "home" | "power";
  glyph?: string;
  x: number;
  y: number;
  width: number;
  height: number;
  round?: boolean;
}

// [nom EADK, libellé, légende shift, lettre alpha, style]
type Item = readonly [KeyName, string, string, string, ("mod" | "small")?];
const ROW_X6 = [133, 286, 439, 592, 745, 898];
const ROW_X5 = [133, 317, 502, 686, 871];
const row = (xs: number[], y: number, width: number, height: number, items: Item[]): CalcKey[] =>
  items.map(([name, label, shift, alpha, style], i) => ({ name, label, shift, alpha, style: style ?? "plain", x: xs[i] ?? 0, y, width, height }));
const row6 = (y: number, items: Item[]) => row(ROW_X6, y, 129, 87, items);
const row5 = (y: number, items: Item[]) => row(ROW_X5, y, 156, 101, items);

const PLAIN = [
  ...row6(1210, [["shift", "shift", "", "", "mod"], ["alpha", "alpha", "lock", "", "mod"], ["xnt", "x,n,t", "cut", ":"], ["var", "var", "copy", ";"], ["toolbox", "toolbox", "paste", '"', "small"], ["backspace", "⌫", "clear", "%"]]),
  ...row6(1332, [["exp", "eˣ", "[", "A"], ["ln", "ln", "]", "B"], ["log", "log", "{", "C"], ["imaginary", "i", "}", "D"], ["comma", ",", "_", "E"], ["power", "xʸ", "sto→", "F"]]),
  ...row6(1455, [["sine", "sin", "asin", "G"], ["cosine", "cos", "acos", "H"], ["tangent", "tan", "atan", "I"], ["pi", "π", "=", "J"], ["sqrt", "√", "<", "K"], ["square", "x²", ">", "L"]]),
  ...row5(1578, [["seven", "7", "", "M"], ["eight", "8", "", "N"], ["nine", "9", "", "O"], ["left_parenthesis", "(", "", "P"], ["right_parenthesis", ")", "", "Q"]]),
  ...row5(1715, [["four", "4", "", "R"], ["five", "5", "", "S"], ["six", "6", "", "T"], ["multiplication", "×", "", "U"], ["division", "÷", "", "V"]]),
  ...row5(1851, [["one", "1", "", "W"], ["two", "2", "", "X"], ["three", "3", "", "Y"], ["plus", "+", "", "Z"], ["minus", "−", "", ""]]),
  ...row5(1988, [["zero", "0", "", "?"], ["dot", ".", "", "!"], ["ee", "×10ˣ", "", ""], ["ans", "Ans", "", ""], ["exe", "EXE", "", ""]]),
];

/** Toutes les touches. */
export const KEYS: CalcKey[] = [
  ...PLAIN,
  // Navigation : croix à gauche, home et on/off au milieu, OK et back à droite.
  { name: "left", label: "◀", style: "nav", x: 133, y: 983, width: 140, height: 87 },
  { name: "up", label: "▲", style: "nav", x: 231, y: 886, width: 87, height: 140 },
  { name: "down", label: "▼", style: "nav", x: 231, y: 1026, width: 87, height: 140 },
  { name: "right", label: "▶", style: "nav", x: 275, y: 983, width: 140, height: 87 },
  { name: "ok", label: "OK", style: "plain", x: 745, y: 963, width: 129, height: 129, round: true },
  { name: "back", label: "back", style: "plain", glyph: GLYPHS.back, x: 898, y: 963, width: 129, height: 129, round: true },
  { name: "home", label: "home", style: "home", glyph: GLYPHS.home, x: 502, y: 908, width: 156, height: 101 },
  { name: "on_off", label: "on/off", style: "power", glyph: GLYPHS.power, x: 502, y: 1044, width: 156, height: 101 },
];

export const KEY_BY_NAME: Record<string, CalcKey | undefined> = Object.fromEntries(KEYS.map((k) => [k.name, k]));
