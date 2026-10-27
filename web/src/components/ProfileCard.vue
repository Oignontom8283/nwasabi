<script setup lang="ts">
// Profil d'exécution : l'émulateur relève régulièrement l'instruction en cours ; les fonctions et adresses qui reviennent le
// plus souvent sont celles où l'application passe son temps.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { showInDebugger } from "../ui.js";
import { assignIfChanged, hex0x, poll } from "../util.js";
import AddressLink from "./AddressLink.vue";

const { t, n } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

interface Report {
  running: boolean;
  total: number;
  functions: Array<{ name: string; start: number; samples: number }>;
  addresses: Array<{ address: number; samples: number; symbol?: string }>;
}
const report = ref<Report | null>(null);
let stopPolling = () => {};

async function refresh() {
  if (!state.loaded) return void (report.value = null);
  const r = await quiet({ cmd: "profile", action: "get", top: 15 });
  assignIfChanged(report, r.ok ? (r as unknown as Report) : null);
}
onMounted(() => {
  stopPolling = poll(refresh, 700);
});
onBeforeUnmount(() => stopPolling());

const running = computed(() => report.value?.running === true);
const total = computed(() => report.value?.total ?? 0);
const percent = (samples: number) => (total.value > 0 ? (100 * samples) / total.value : 0);

async function start() {
  await quiet({ cmd: "profile", action: "start" });
  await refresh();
}
async function stop() {
  await quiet({ cmd: "profile", action: "stop" });
  await refresh();
}
</script>

<template>
  <div class="profile">
    <div class="head">
      <p class="muted help">{{ t("profile.help") }}</p>
      <button v-if="!running" :disabled="!state.loaded" @click="start">{{ t("profile.start") }}</button>
      <button v-else @click="stop">{{ t("profile.stop") }}</button>
    </div>
    <template v-if="report && running && total > 0">
      <p class="muted small">{{ t("profile.samples", { n: n(total) }, total) }}</p>
      <div class="gh">
        <div class="gh-head cols" aria-hidden="true"><span>{{ t("profile.function") }}</span><span>{{ t("profile.share") }}</span><span /></div>
        <div class="gh-rows">
          <div v-for="f in report.functions" :key="f.start" class="gh-row cols">
            <AddressLink :address="f.start" :label="f.name" view="code" />
            <span class="bar"><span :style="{ width: percent(f.samples) + '%' }" /></span>
            <span class="mono pct">{{ percent(f.samples).toFixed(1) }} %</span>
          </div>
        </div>
      </div>
      <div class="gh">
        <div class="gh-head cols" aria-hidden="true"><span>{{ t("profile.address") }}</span><span>{{ t("profile.share") }}</span><span /></div>
        <div class="gh-rows">
          <div v-for="a in report.addresses" :key="a.address" class="gh-row cols">
            <AddressLink :address="a.address" view="code" />
            <span class="bar"><span :style="{ width: percent(a.samples) + '%' }" /></span>
            <span class="mono pct">{{ percent(a.samples).toFixed(1) }} %</span>
          </div>
        </div>
      </div>
    </template>
    <p v-else-if="running" class="muted small">{{ t("profile.waiting") }}</p>
  </div>
</template>

<style scoped>
.profile { display: grid; gap: 8px; }
.head { display: flex; gap: 12px; align-items: flex-start; justify-content: space-between; }
.head p { margin: 0; }
.help, .small { font-size: 12px; margin: 0; }
.cols { grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) 56px; }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; overflow: hidden; text-overflow: ellipsis; }
.link:hover { text-decoration: underline; }
.bar { height: 8px; background: var(--surface-2); border: 1px solid var(--line); border-radius: 5px; overflow: hidden; align-self: center; }
.bar span { display: block; height: 100%; background: var(--accent); }
.pct { text-align: right; }
.gh-rows { max-height: 260px; overflow: auto; }
</style>
