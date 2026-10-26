// État de la mise en page, partagé entre les vues (et mémorisé quand c'est utile).

import { reactive, watch } from "vue";
import { remember } from "./util.js";

export const TABS = ["files", "memory", "structures", "keys", "settings", "trace"];

export const ui = reactive({
  /** Grand panneau latéral déployé ? */
  panelOpen: remember.get("nwasabi.panelOpen", false),
  /** Largeur du grand panneau, en pixels (corrigée à la taille de la fenêtre par SidePanel). */
  panelWidth: remember.get("nwasabi.panelWidth", 0),
  /** Petite barre d'infos visible quand le grand panneau est fermé ? */
  miniVisible: remember.get("nwasabi.miniVisible", true),
  tab: remember.get("nwasabi.tab", "files"),
  /** Écran en plein écran ? */
  fullscreen: false,
  /** Thème : "auto", "light" ou "dark". */
  theme: remember.get("nwasabi.theme", "auto"),
  /** Largeur de la fenêtre (suivie par l'application). */
  winWidth: innerWidth,
  /** Demande de l'onglet Mémoire : adresse à montrer dans l'éditeur hexadécimal. */
  hexGoto: null,
  /** Demande de l'onglet Structures : section à montrer. */
  structureGoto: null,
});

watch(() => ui.panelOpen, (v) => remember.set("nwasabi.panelOpen", v));
// La largeur change à chaque image pendant un glissement : on n'écrit dans le stockage qu'une fois qu'elle se stabilise.
let widthTimer = 0;
watch(
  () => ui.panelWidth,
  (v) => {
    clearTimeout(widthTimer);
    widthTimer = setTimeout(() => remember.set("nwasabi.panelWidth", v), 300);
  },
);

export const PANEL_MIN = 340;
export const PANEL_ROOM = 280; // largeur laissée à la calculatrice
/** Largeur du grand panneau en pixels, ramenée à la taille de la fenêtre. */
export function panelPixels() {
  const wanted = ui.panelWidth || Math.round(ui.winWidth / 2);
  return Math.max(PANEL_MIN, Math.min(wanted, ui.winWidth - PANEL_ROOM));
}
watch(() => ui.miniVisible, (v) => remember.set("nwasabi.miniVisible", v));
watch(() => ui.tab, (v) => remember.set("nwasabi.tab", v));
watch(
  () => ui.theme,
  (v) => {
    remember.set("nwasabi.theme", v);
    applyTheme(v);
  },
);

export function applyTheme(theme = ui.theme) {
  const root = document.documentElement;
  if (theme === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/** Va à l'onglet Mémoire, à cette adresse. */
export function showInMemory(address) {
  ui.hexGoto = { address, seq: Date.now() };
  ui.tab = "memory";
  ui.panelOpen = true;
}

/** Va à l'onglet Structures, à cette section. */
export function showStructure(section) {
  ui.structureGoto = { section, seq: Date.now() };
  ui.tab = "structures";
  ui.panelOpen = true;
}

/** Fonctions de l'écran, enregistrées par `ScreenView` (plein écran, capture). */
export const screenApi = {
  toggleFullscreen: () => {},
  exitFullscreen: () => {},
  capture: () => {},
};
