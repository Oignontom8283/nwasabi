// État de la mise en page, partagé entre les vues (et mémorisé quand c'est utile).

import { inject, reactive, watch } from "vue";
import { remember } from "./util.js";

export type Theme = "auto" | "light" | "dark";
export type DebugView = "code" | "watch" | "profile" | "calls";
export const TABS = ["files", "memory", "structures", "keys", "settings", "debug", "trace"];

export const ui = reactive({
  /** Grand panneau latéral déployé ? */
  panelOpen: remember.get("nwasabi.panelOpen", false),
  /** Largeur du grand panneau, en pixels (corrigée à la taille de la fenêtre par SidePanel). */
  panelWidth: remember.get("nwasabi.panelWidth", 0),
  /** Petite barre d'infos visible quand le grand panneau est fermé ? */
  miniVisible: remember.get("nwasabi.miniVisible", true),
  tab: remember.get<string>("nwasabi.tab", "files"),
  /** Recharger tout seul ce que le serveur donne quand le fichier change sur le disque ? */
  autoReload: remember.get("nwasabi.autoReload", false),
  /** Assombrir l'écran selon la luminosité réglée par l'application (rétroéclairage) ? */
  backlight: remember.get("nwasabi.backlight", true),
  /** Écran agrandi (sans la calculatrice autour, mais pas en plein écran) ? */
  large: remember.get("nwasabi.large", false),
  /** Écran en plein écran ? */
  fullscreen: false,
  /** Thème : "auto", "light" ou "dark". */
  theme: remember.get<Theme>("nwasabi.theme", "auto"),
  /** Largeur de la fenêtre (suivie par l'application). */
  winWidth: innerWidth,
  /** Demande de l'onglet Mémoire : adresse à montrer dans l'éditeur hexadécimal. */
  hexGoto: null as { address: number; seq: number } | null,
  /** Outil montré dans l'onglet Débogage. */
  debugView: remember.get<DebugView>("nwasabi.debugView", "code"),
  /** Demande de l'onglet Débogueur : adresse à montrer dans le code. */
  debugGoto: null as { address: number; seq: number } | null,
  /** Demande de l'onglet Structures : section à montrer. */
  structureGoto: null as { section: string; seq: number } | null,
});

watch(() => ui.panelOpen, (v) => remember.set("nwasabi.panelOpen", v));
// La largeur change à chaque image pendant un glissement : on n'écrit dans le stockage qu'une fois qu'elle se stabilise.
let widthTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => ui.panelWidth,
  (v) => {
    clearTimeout(widthTimer);
    widthTimer = setTimeout(() => remember.set("nwasabi.panelWidth", v), 300);
  },
);

/** Écran étroit (téléphone, tablette en hauteur) : le panneau recouvre la calculatrice au lieu de se mettre à côté. */
export const NARROW = 760;
export const isNarrow = () => ui.winWidth < NARROW;

export const PANEL_MIN = 340;
export const PANEL_ROOM = 280; // largeur laissée à la calculatrice
/** Largeur du grand panneau en pixels, ramenée à la taille de la fenêtre. */
export function panelPixels() {
  if (isNarrow()) return ui.winWidth;
  const wanted = ui.panelWidth || Math.round(ui.winWidth / 2);
  return Math.max(PANEL_MIN, Math.min(wanted, ui.winWidth - PANEL_ROOM));
}
watch(() => ui.miniVisible, (v) => remember.set("nwasabi.miniVisible", v));
watch(() => ui.autoReload, (v) => remember.set("nwasabi.autoReload", v));
watch(() => ui.backlight, (v) => remember.set("nwasabi.backlight", v));
watch(() => ui.large, (v) => remember.set("nwasabi.large", v));
watch(() => ui.debugView, (v) => remember.set("nwasabi.debugView", v));
watch(() => ui.tab, (v) => remember.set("nwasabi.tab", v));
watch(
  () => ui.theme,
  (v) => {
    remember.set("nwasabi.theme", v);
    applyTheme(v);
  },
);

export function applyTheme(theme: Theme = ui.theme): void {
  const root = document.documentElement;
  if (theme === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/** Va à l'onglet Mémoire, à cette adresse. */
export function showInMemory(address: number): void {
  ui.hexGoto = { address, seq: Date.now() };
  ui.tab = "memory";
  ui.panelOpen = true;
}

/** Va à l'onglet Débogueur, à cette adresse. */
export function showInDebugger(address: number): void {
  ui.debugGoto = { address, seq: Date.now() };
  ui.debugView = "code";
  ui.tab = "debug";
  ui.panelOpen = true;
}

/** Va à l'onglet Structures, à cette section. */
export function showStructure(section: string): void {
  ui.structureGoto = { section, seq: Date.now() };
  ui.tab = "structures";
  ui.panelOpen = true;
}

/** Fonctions de l'écran, enregistrées par `ScreenView` (plein écran, capture). */
export const screenApi = {
  toggleFullscreen: () => {},
  exitFullscreen: () => {},
  capture: () => {},
  copy: () => {},
  toggleRecording: () => {},
};

/** Un GIF est en cours d'enregistrement ? */
export const recording = reactive({ on: false, since: 0 });

/** Un petit message qui s'efface (« État enregistré »…), affiché en bas de la page. */
export const notice = reactive({ text: "", seq: 0 });
let noticeTimer: ReturnType<typeof setTimeout> | undefined;
export function notify(text: string, ms = 2200): void {
  notice.text = text;
  notice.seq++;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (notice.text = ""), ms);
}

/** Ce que le menu de chargement peut ouvrir : un sélecteur de fichier par sorte de fichier. */
export type PickKind = "app" | "data" | "keys" | "state" | "files";

/** La fonction qui ouvre un sélecteur de fichier, fournie par `App.vue`. */
export function usePick(): (which: PickKind) => void {
  const pick = inject<(which: PickKind) => void>("pick");
  if (!pick) throw new Error("« pick » non fourni");
  return pick;
}
