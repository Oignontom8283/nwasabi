// Les raccourcis clavier de l'interface : un seul endroit pour les noms affichés (infobulles, palette de commandes) et pour les
// touches que `App.vue` écoute.

import { matchKeybindingPress, parseKeybinding } from "tinykeys";
import type { KeybindingPress } from "tinykeys";

export type ShortcutId = "toggleRun" | "frame" | "restart" | "capture" | "copy" | "gif" | "video" | "fullscreen" | "large" | "panel" | "quickSave" | "quickLoad" | "reload" | "palette";

/** Nom affiché de chaque raccourci. Ctrl (Cmd sur Mac) : des combinaisons que les navigateurs laissent intercepter, sans toucher à Ctrl + C, V, X, A ni aux touches réservées (Ctrl + N, T, W). */
export const SHORTCUTS: Record<ShortcutId, string> = {
  toggleRun: "Ctrl + Espace",
  frame: "Ctrl + .",
  restart: "Ctrl + Maj + Espace",
  capture: "Ctrl + E",
  copy: "Ctrl + Maj + E",
  gif: "Ctrl + G",
  video: "Ctrl + Maj + G",
  fullscreen: "Ctrl + Entrée",
  large: "Ctrl + B",
  panel: "Ctrl + J",
  quickSave: "Ctrl + S",
  quickLoad: "Ctrl + O",
  reload: "Ctrl + R",
  palette: "Ctrl + K",
};

/** Ajoute le raccourci à une infobulle : « Reprend la simulation. (Alt + P) ». */
export const keyed = (text: string, id: ShortcutId): string => `${text} (${SHORTCUTS[id]})`;

/**
 * Les combinaisons reconnues, dans la syntaxe de `tinykeys` : Ctrl (ou Cmd) + touche, et les anciennes combinaisons Alt + touche
 * (certains navigateurs gardent pour eux telle ou telle combinaison avec Ctrl). Les touches sont repérées par leur position
 * (`KeyE`, `Space`…) : elles ne dépendent pas de la disposition du clavier. Les modificateurs doivent être exactement ceux
 * demandés (Ctrl + Maj + S n'est pas Ctrl + S).
 */
const BINDINGS: Array<[ShortcutId, string[]]> = [
  ["toggleRun", ["Control+Space", "Meta+Space", "Alt+KeyP"]],
  ["restart", ["Control+Shift+Space", "Meta+Shift+Space", "Alt+Shift+KeyR"]],
  ["frame", ["Control+Period", "Meta+Period", "Alt+KeyN"]],
  ["capture", ["Control+KeyE", "Meta+KeyE", "Alt+KeyC"]],
  ["copy", ["Control+Shift+KeyE", "Meta+Shift+KeyE", "Alt+Shift+KeyC"]],
  ["gif", ["Control+KeyG", "Meta+KeyG", "Alt+KeyG"]],
  ["video", ["Control+Shift+KeyG", "Meta+Shift+KeyG", "Alt+Shift+KeyG"]],
  ["fullscreen", ["Control+Enter", "Meta+Enter", "Alt+Enter"]],
  ["large", ["Control+KeyB", "Meta+KeyB", "Alt+KeyE"]],
  ["panel", ["Control+KeyJ", "Meta+KeyJ", "Alt+KeyT"]],
  ["quickSave", ["Control+KeyS", "Meta+KeyS", "Alt+KeyS"]],
  ["quickLoad", ["Control+KeyO", "Meta+KeyO", "Alt+KeyL"]],
  ["reload", ["Control+KeyR", "Meta+KeyR", "Alt+KeyR"]],
];
const PARSED = BINDINGS.flatMap(([id, combos]) => combos.map((combo) => [id, parseKeybinding(combo)] as const));

/** Le raccourci d'un événement clavier, ou `null` : sert à `App.vue`. */
export function shortcutOf(event: KeyboardEvent): ShortcutId | null {
  return PARSED.find(([, presses]) => presses.length === 1 && matchKeybindingPress(event, presses[0] as KeybindingPress))?.[0] ?? null;
}
