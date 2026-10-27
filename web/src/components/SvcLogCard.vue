<script setup lang="ts">
// Journal des appels système : chaque appel de l'application au système (écran, clavier, temps…), avec ses arguments et sa
// valeur de retour, dans l'ordre. C'est la façon de voir ce que l'application demande vraiment à la calculatrice.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { NAMES } from "../svcNames.js";
import { showInDebugger } from "../ui.js";
import { hex, hex0x, poll } from "../util.js";
import AddressLink from "./AddressLink.vue";

interface Call {
  seq: number;
  svc: number;
  pc: number;
  args: number[];
  ret: number;
  frame: number;
}

const { t, n } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

const MAX = 400;
const running = ref(false);
const calls = ref<Call[]>([]);
const total = ref(0);
const filter = ref("");
const paused = ref(false);
let next = 0;
let stopPolling = () => {};

async function refresh() {
  if (!state.loaded || !running.value || paused.value) return;
  const reply = await quiet({ cmd: "svc_log", since: next });
  if (!reply.ok) return;
  const fresh = reply.calls as Call[];
  total.value = reply.total as number;
  if (fresh.length) {
    next = (fresh[fresh.length - 1] as Call).seq + 1;
    calls.value = [...calls.value, ...fresh].slice(-MAX);
  }
}
onMounted(async () => {
  stopPolling = poll(refresh, 300);
  // Un journal déjà lancé (en revenant sur l'onglet) reste lancé.
  if (state.loaded) running.value = (await quiet({ cmd: "svc_log" })).running === true;
});
onBeforeUnmount(() => stopPolling());

async function start() {
  await quiet({ cmd: "svc_log", action: "start" });
  calls.value = [];
  next = 0;
  running.value = true;
}
async function stop() {
  await quiet({ cmd: "svc_log", action: "stop" });
  running.value = false;
}
function clear() {
  calls.value = [];
}

const nameOf = (svc: number) => NAMES[svc]?.replace(" (nwasabi)", "") ?? `svc ${svc}`;
const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  const list = q ? calls.value.filter((c) => nameOf(c.svc).toLowerCase().includes(q) || String(c.svc) === q) : calls.value;
  return [...list].reverse();
});
const argsText = (c: Call) => c.args.map((a) => hex(a, 1)).join(", ");
</script>

<template>
  <div class="log">
    <div class="head">
      <p class="muted help">{{ t("svclog.help") }}</p>
      <button v-if="!running" :disabled="!state.loaded" @click="start">{{ t("svclog.start") }}</button>
      <button v-else @click="stop">{{ t("svclog.stop") }}</button>
    </div>
    <template v-if="running">
      <div class="bar">
        <input v-model="filter" type="search" class="mono" :placeholder="t('svclog.filter')" :aria-label="t('svclog.filter')" data-keep-keys />
        <label class="check"><input v-model="paused" type="checkbox" /> {{ t("svclog.freeze") }}</label>
        <button :disabled="!calls.length" @click="clear">{{ t("svclog.clear") }}</button>
        <span class="muted small">{{ t("svclog.total", { n: n(total) }, total) }}</span>
      </div>
      <div class="gh">
        <div class="gh-head cols" aria-hidden="true"><span>#</span><span>{{ t("svclog.call") }}</span><span>{{ t("svclog.args") }}</span><span>{{ t("svclog.ret") }}</span><span>{{ t("svclog.from") }}</span></div>
        <div class="gh-rows">
          <div v-for="c in shown" :key="c.seq" class="gh-row cols">
            <span class="mono muted">{{ c.seq }}</span>
            <span class="mono" :title="`svc #${c.svc}`">{{ nameOf(c.svc) }}</span>
            <span class="mono args" :title="argsText(c)">{{ argsText(c) }}</span>
            <span class="mono">{{ hex(c.ret, 1) }}</span>
            <AddressLink :address="c.pc" view="code" />
          </div>
          <p v-if="!shown.length" class="empty muted">{{ t("svclog.none") }}</p>
        </div>
      </div>
      <p class="muted small">{{ t("svclog.argsNote") }}</p>
    </template>
  </div>
</template>

<style scoped>
.log { display: grid; gap: 8px; }
.head { display: flex; gap: 12px; align-items: flex-start; justify-content: space-between; }
.head p, .help, .small { margin: 0; font-size: 12px; }
.bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.bar input[type="search"] { width: 12em; }
.check { display: flex; gap: 4px; align-items: center; font-size: 12px; }
.cols { grid-template-columns: 44px minmax(0, 1.5fr) minmax(0, 1.6fr) 70px 96px; }
.args { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; }
.link:hover { text-decoration: underline; }
.empty { padding: 14px; text-align: center; margin: 0; }
.gh-rows { max-height: 280px; overflow: auto; }
</style>
