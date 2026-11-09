<script setup lang="ts">
// Console de commandes à la GDB : mêmes actions que les autres vues, au clavier, dans un terminal (xterm.js, avec son apparence par défaut).
// xterm.js est chargé à l'ouverture de cette vue seulement : il ne pèse pas sur le démarrage de l'émulateur.
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { Terminal } from "@xterm/xterm";
import { useEmu } from "../../emulator.js";
import { runCommand } from "../../gdbConsole.js";
import { source, sourceLines } from "../../sourceStore.js";
import type { Backend } from "../../gdbConsole.js";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const PROMPT = "(nwasabi) ";
const host = ref<HTMLElement | null>(null);
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

async function open() {
  if (!host.value) return;
  const [{ Terminal }, { FitAddon }] = await Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit"), import("@xterm/xterm/css/xterm.css")]);
  if (disposed || !host.value || term) return;
  term = new Terminal({ rows: 22, cursorBlink: true, convertEol: false, scrollback: 2000, fontSize: 13 });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(host.value);
  fit.fit();
  resize = new ResizeObserver(() => fit.fit());
  resize.observe(host.value);
  term.onData(onData);
  if (!consoleLog.length) consoleLog.push(t("console.welcome"));
  for (const text of consoleLog) print(text, false);
  prompt();
  term.focus();
}

// Le terminal s'ouvre dès qu'il y a une application (la zone n'existe pas avant).
onMounted(() => void openWhenLoaded());
watch(() => state.loaded, () => void openWhenLoaded());
async function openWhenLoaded() {
  await nextTick();
  if (state.loaded && !term) await open();
}
onBeforeUnmount(() => {
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
      <div ref="host" class="terminal" role="log" :aria-label="t('console.label')" data-keep-keys />
      <p class="muted small">{{ t("console.hint") }} <button class="link" @click="run('help')">{{ t("console.help") }}</button> · <button class="link" @click="clear">{{ t("console.clear") }}</button></p>
    </template>
  </div>
</template>

<style scoped>
.console { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
.terminal { min-width: 0; padding: 4px; background: #000; border-radius: 8px; overflow: hidden; }
.small { margin: 0; font-size: 12px; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
