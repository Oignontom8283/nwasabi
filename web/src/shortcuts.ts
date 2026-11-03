// Les raccourcis clavier de l'interface : un seul endroit pour les noms affichés (infobulles, palette de commandes) et pour les
// touches que `App.vue` écoute.

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
 * Le raccourci d'un événement clavier, ou `null` : sert à `App.vue`. Ctrl (ou Cmd) + touche ; les anciennes combinaisons Alt + touche
 * restent acceptées (certains navigateurs gardent pour eux telle ou telle combinaison avec Ctrl).
 */
export function shortcutOf(event: KeyboardEvent): ShortcutId | null {
  const ctrl = (event.ctrlKey || event.metaKey) && !event.altKey;
  const alt = event.altKey && !event.ctrlKey && !event.metaKey;
  if (!ctrl && !alt) return null;
  const shift = event.shiftKey;
  if (ctrl) {
    switch (event.code) {
      case "Space": return shift ? "restart" : "toggleRun";
      case "Period": return "frame";
      case "KeyE": return shift ? "copy" : "capture";
      case "KeyG": return shift ? "video" : "gif";
      case "Enter": return "fullscreen";
      case "KeyB": return "large";
      case "KeyJ": return "panel";
      case "KeyS": return shift ? null : "quickSave";
      case "KeyO": return "quickLoad";
      case "KeyR": return shift ? null : "reload";
      default: return null;
    }
  }
  switch (event.code) {
    case "KeyP": return "toggleRun";
    case "KeyN": return "frame";
    case "KeyR": return shift ? "restart" : "reload";
    case "KeyC": return shift ? "copy" : "capture";
    case "KeyG": return shift ? "video" : "gif";
    case "Enter": return "fullscreen";
    case "KeyE": return "large";
    case "KeyT": return "panel";
    case "KeyS": return "quickSave";
    case "KeyL": return "quickLoad";
    default: return null;
  }
}
