<script setup lang="ts">
// Console de commandes à la GDB : mêmes actions que les autres vues, au clavier.
import { nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { runCommand } from "../../gdbConsole.js";
import type { Backend } from "../../gdbConsole.js";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

interface Entry {
  kind: "in" | "out";
  text: string;
}
// L'historique reste en mémoire tant que la page est ouverte (il survit à un changement d'onglet).
const log = ref<Entry[]>(consoleLog);
const line = ref("");
const history: string[] = consoleHistory;
let at = history.length;
const scroller = ref<HTMLElement | null>(null);
const busy = ref(false);

const backend: Backend = {
  request: (request) => emulator.quiet(request as never) as never,
  isRunning: () => state.running,
  loaded: () => state.loaded,
  resume: () => emulator.start() as Promise<void>,
  pause: () => emulator.stop(),
  refresh: () => emulator.bump(),
};

async function scrollDown() {
  await nextTick();
  if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight;
}

async function submit() {
  const text = line.value.trim();
  line.value = "";
  if (!text || busy.value) return;
  history.push(text);
  at = history.length;
  log.value.push({ kind: "in", text });
  busy.value = true;
  const out = await runCommand(text, backend);
  busy.value = false;
  for (const o of out) log.value.push({ kind: "out", text: o });
  if (log.value.length > 1000) log.value.splice(0, log.value.length - 1000);
  await scrollDown();
}

function key(event: KeyboardEvent) {
  if (event.key === "ArrowUp" && at > 0) line.value = history[--at] ?? "";
  else if (event.key === "ArrowDown") line.value = at < history.length - 1 ? (history[++at] ?? "") : ((at = history.length), "");
  else return;
  event.preventDefault();
}

onMounted(() => {
  if (!log.value.length) log.value.push({ kind: "out", text: t("console.welcome") });
  void scrollDown();
});
const showHelp = () => {
  line.value = "help";
  void submit();
};
</script>

<script lang="ts">
// Hors du composant : ce que la console a déjà affiché ne disparaît pas quand on change d'onglet.
const consoleLog: Array<{ kind: "in" | "out"; text: string }> = [];
const consoleHistory: string[] = [];
</script>

<template>
  <div class="console">
    <p v-if="!state.loaded" class="empty muted">{{ t("debug.noApp") }}</p>
    <template v-else>
      <div ref="scroller" class="screen mono" role="log" :aria-label="t('console.label')" tabindex="0">
        <div v-for="(e, i) in log" :key="i" class="line" :class="e.kind">
          <span v-if="e.kind === 'in'" class="prompt">(nwasabi)</span> {{ e.text }}
        </div>
      </div>
      <form class="input" @submit.prevent="submit">
        <span class="prompt mono">(nwasabi)</span>
        <input v-model="line" class="mono" :placeholder="t('console.placeholder')" :aria-label="t('console.placeholder')" spellcheck="false" autocomplete="off" data-keep-keys @keydown="key" />
        <button type="submit" :disabled="busy">{{ t("console.run") }}</button>
      </form>
      <p class="muted small">{{ t("console.hint") }} <button class="link" @click="showHelp">{{ t("console.help") }}</button> · <button class="link" @click="log.splice(0)">{{ t("console.clear") }}</button></p>
    </template>
  </div>
</template>

<style scoped>
.console { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
.screen { height: 380px; overflow: auto; padding: 8px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); font-size: 12.5px; white-space: pre; tab-size: 8; }
.line { min-width: max-content; }
.line.in { font-weight: 600; }
.line.out { color: var(--text); }
.prompt { color: var(--accent-strong); }
.input { display: flex; gap: 8px; align-items: center; }
.input input { flex: 1; min-width: 0; }
.small { margin: 0; font-size: 12px; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
