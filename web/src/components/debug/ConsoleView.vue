<script setup lang="ts">
// Console de commandes à la GDB : mêmes actions que les autres vues, au clavier, dans un terminal (xterm.js, avec son apparence par défaut).
// xterm.js est chargé à l'ouverture de cette vue seulement : il ne pèse pas sur le démarrage de l'émulateur.
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { FitAddon } from "@xterm/addon-fit";
import { useI18n } from "vue-i18n";
import type { Terminal } from "@xterm/xterm";
import { useEmu } from "../../emulator.js";
import { runCommand } from "../../gdbConsole.js";
import { source, sourceLines } from "../../sourceStore.js";
import { ui } from "../../ui.js";
import { remember } from "../../util.js";
import type { Backend } from "../../gdbConsole.js";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

/** L'invite, en vert et en gras (les séquences `\x1b[…m` sont des couleurs du terminal). */
const PROMPT = "\x1b[1;32m(nwasabi)\x1b[0m ";
/** Largeur du terminal en colonnes : assez large pour qu'une ligne ne soit jamais coupée ; au-delà de l'écran, on fait défiler. */
const COLS = 240;
const MIN_HEIGHT = 120;
/** Hauteur de la zone, réglée par le poignée en bas à droite et gardée d'une visite à l'autre. */
const height = ref(remember.get("nwasabi.consoleHeight", 380));
let fit: FitAddon | null = null;
/** Fond de la zone (celui du terminal, pour que le pourtour ait la même couleur). */
const background = ref("");
const host = ref<HTMLElement | null>(null);
const frame = ref<HTMLElement | null>(null);
const history: string[] = consoleHistory;
let term: Terminal | null = null;
let resize: ResizeObserver | null = null;
let disposed = false;
let line = "";
let cursor = 0;
let at = history.length;
let busy = false;

const backend: Backend = {
  request: (request) => emulator.quiet(request as never) as never,
  isRunning: () => state.running,
  loaded: () => state.loaded,
  resume: () => emulator.start() as Promise<void>,
  pause: () => emulator.stop(),
  refresh: () => emulator.bump(),
  sourceText: (path) => sourceLines(path),
  frame: { get: () => source.frame, set: (index) => void (source.frame = index) },
};

/** Écrit du texte (les fins de ligne `\n` deviennent `\r\n`) et le garde pour la prochaine ouverture de la vue. */
function print(text: string, remember = true) {
  if (text.startsWith("Erreur")) text = `\x1b[31m${text}\x1b[0m`;
  if (remember) consoleLog.push(text);
  if (consoleLog.length > 1000) consoleLog.splice(0, consoleLog.length - 1000);
  term?.write(`${text.replace(/\r?\n/g, "\r\n")}\r\n`);
}

/** Réaffiche la ligne en cours de saisie, curseur à sa place. */
function redraw() {
  term?.write(`\r\x1b[2K${PROMPT}${line}`);
  if (cursor < line.length) term?.write(`\x1b[${line.length - cursor}D`);
}

async function submit() {
  const text = line.trim();
  term?.write("\r\n");
  line = "";
  cursor = 0;
  if (!text) return prompt();
  history.push(text);
  at = history.length;
  consoleLog.push(`${PROMPT}${text}`);
  busy = true;
  const out = await runCommand(text, backend);
  busy = false;
  if (disposed) return;
  for (const o of out) print(o);
  prompt();
}

function prompt() {
  term?.write(PROMPT);
}

function onData(data: string) {
  if (busy) return;
  switch (data) {
    case "\r":
      return void submit();
    case "\x7f":
      if (cursor > 0) {
        line = line.slice(0, cursor - 1) + line.slice(cursor);
        cursor--;
        redraw();
      }
      return;
    case "\x1b[A":
      if (at > 0) {
        line = history[--at] ?? "";
        cursor = line.length;
        redraw();
      }
      return;
    case "\x1b[B":
      line = at < history.length - 1 ? (history[++at] ?? "") : ((at = history.length), "");
      cursor = line.length;
      return redraw();
    case "\x1b[D":
      if (cursor > 0) {
        cursor--;
        term?.write("\x1b[D");
      }
      return;
    case "\x1b[C":
      if (cursor < line.length) {
        cursor++;
        term?.write("\x1b[C");
      }
      return;
    case "\x03":
      line = "";
      cursor = 0;
      term?.write("^C\r\n");
      return prompt();
  }
  // Texte saisi ou collé (sans caractères de contrôle).
  const text = data.replace(/[\x00-\x1f\x7f]/g, "");
  if (!text) return;
  line = line.slice(0, cursor) + text + line.slice(cursor);
  cursor += text.length;
  redraw();
}

/** Les palettes des terminaux de Visual Studio Code : « Dark+ » et « Light+ ». */
const DARK = {
  background: "#1e1e1e", foreground: "#cccccc", cursor: "#aeafad", cursorAccent: "#1e1e1e", selectionBackground: "#264f78",
  black: "#000000", red: "#cd3131", green: "#0dbc79", yellow: "#e5e510", blue: "#2472c8", magenta: "#bc3fbc", cyan: "#11a8cd", white: "#e5e5e5",
  brightBlack: "#666666", brightRed: "#f14c4c", brightGreen: "#23d18b", brightYellow: "#f5f543", brightBlue: "#3b8eea", brightMagenta: "#d670d6", brightCyan: "#29b8db", brightWhite: "#e5e5e5",
};
const LIGHT = {
  background: "#ffffff", foreground: "#333333", cursor: "#000000", cursorAccent: "#ffffff", selectionBackground: "#add6ff",
  black: "#000000", red: "#cd3131", green: "#1b7f3b", yellow: "#949800", blue: "#0451a5", magenta: "#bc05bc", cyan: "#0598bc", white: "#555555",
  brightBlack: "#666666", brightRed: "#cd3131", brightGreen: "#1b7f3b", brightYellow: "#b5ba00", brightBlue: "#0451a5", brightMagenta: "#bc05bc", brightCyan: "#0598bc", brightWhite: "#a5a5a5",
};

/** Le thème de la page : celui choisi par l'utilisateur, sinon celui du système. */
function colors() {
  const dark = ui.theme === "dark" || (ui.theme === "auto" && matchMedia("(prefers-color-scheme: dark)").matches);
  return dark ? DARK : LIGHT;
}

/** Ajuste le nombre de lignes à la hauteur de la zone, sans toucher aux colonnes. */
function fitRows() {
  const rows = fit?.proposeDimensions()?.rows;
  if (term && rows && rows !== term.rows) term.resize(COLS, rows);
}

/**
 * Copier et coller. Avec du texte sélectionné, Ctrl + C (ou Ctrl + Maj + C, ou Cmd + C) le copie : sans sélection, Ctrl + C reste
 * l'interruption de la saisie. Ctrl + V et Ctrl + Maj + V collent (le navigateur donne le texte à xterm). Ctrl + Maj + C ne doit
 * pas ouvrir les outils du navigateur.
 */
function onKey(event: KeyboardEvent): boolean {
  if (event.type !== "keydown") return true;
  const modifier = event.ctrlKey || event.metaKey;
  const key = event.key.toLowerCase();
  if (modifier && key === "c") {
    const selection = term?.getSelection() ?? "";
    if (selection) {
      void navigator.clipboard?.writeText(selection).catch(() => {});
      term?.clearSelection();
      event.preventDefault();
      return false;
    }
    if (event.shiftKey) {
      event.preventDefault();
      return false;
    }
    return true; // Ctrl + C sans sélection : interruption (traitée par onData)
  }
  // Coller : on laisse le navigateur faire, xterm reçoit le texte collé.
  if (modifier && key === "v") return false;
  return true;
}

async function open() {
  if (!host.value) return;
  const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit"), import("@xterm/xterm/css/xterm.css")]);
  if (disposed || !host.value || term) return;
  term = new Terminal({ cols: COLS, rows: 22, cursorBlink: true, convertEol: false, scrollback: 2000, fontSize: 13, theme: colors() });
  fit = new FitAddon();
  term.loadAddon(fit);
  term.open(host.value);
  fitRows();
  // Seule la hauteur s'adapte à la zone ; la largeur reste de COLS colonnes (pas de retour à la ligne).
  resize = new ResizeObserver(() => {
    height.value = Math.max(MIN_HEIGHT, Math.round(frame.value?.getBoundingClientRect().height ?? height.value));
    fitRows();
  });
  if (frame.value) resize.observe(frame.value);
  term.onData(onData);
  term.attachCustomKeyEventHandler(onKey);
  if (!consoleLog.length) consoleLog.push(t("console.welcome"));
  for (const text of consoleLog) print(text, false);
  prompt();
  term.focus();
}

// Le terminal s'ouvre dès qu'il y a une application (la zone n'existe pas avant).
onMounted(() => {
  background.value = colors().background;
  void openWhenLoaded();
});
watch(() => state.loaded, () => void openWhenLoaded());
async function openWhenLoaded() {
  await nextTick();
  if (state.loaded && !term) await open();
}
// Le thème change (choix de l'utilisateur ou réglage du système) : le terminal reprend les couleurs de la page.
const media = matchMedia("(prefers-color-scheme: dark)");
const retheme = () =>
  void nextTick(() => {
    background.value = colors().background;
    if (term) term.options.theme = colors();
  });
watch(() => ui.theme, retheme);
media.addEventListener("change", retheme);
onBeforeUnmount(() => {
  media.removeEventListener("change", retheme);
  remember.set("nwasabi.consoleHeight", height.value);
  disposed = true;
  resize?.disconnect();
  term?.dispose();
  term = null;
});

const run = (text: string) => {
  if (busy) return;
  line = text;
  cursor = text.length;
  redraw();
  void submit();
};
const clear = () => {
  consoleLog.splice(0);
  term?.reset();
  prompt();
};
</script>

<script lang="ts">
// Hors du composant : ce que la console a déjà affiché ne disparaît pas quand on change d'onglet.
const consoleLog: string[] = [];
const consoleHistory: string[] = [];
</script>

<template>
  <div class="console">
    <p v-if="!state.loaded" class="empty muted">{{ t("debug.noApp") }}</p>
    <template v-else>
      <div ref="frame" class="frame" :style="{ height: height + 'px', background }"><div ref="host" class="terminal" role="log" :aria-label="t('console.label')" data-keep-keys /></div>
      <p class="muted small">{{ t("console.hint") }} <button class="link" @click="run('help')">{{ t("console.help") }}</button> · <button class="link" @click="clear">{{ t("console.clear") }}</button></p>
    </template>
  </div>
</template>

<style scoped>
.console { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
/* La zone : sa hauteur se règle à la poignée en bas à droite ; plus large que l'écran, elle défile en travers (pas de retour à la ligne). */
.frame { min-width: 0; min-height: 120px; resize: vertical; overflow-x: auto; overflow-y: hidden; border: 1px solid var(--line); border-radius: 3px; }
.terminal { height: 100%; width: max-content; padding: 4px; }
.small { margin: 0; font-size: 12px; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
