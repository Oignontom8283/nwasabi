<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { Reply } from "@nwasabi/js";
import { choiceDialog } from "../../dialogs.js";
import { quick, quickLoad, quickSave } from "../../quicksave.js";
import { download, formatBytes, hex0x, stem, poll, assignIfChanged } from "../../util.js";
import Icon from "../Icon.vue";
import HexEditor from "../HexEditor.vue";
import type { HexRegion } from "../HexEditor.vue";
import MemoryMap from "../MemoryMap.vue";
import MemoryScanner from "../MemoryScanner.vue";
import BookmarksCard from "../BookmarksCard.vue";
import WatchPoints from "../WatchPoints.vue";

const { t, locale } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

const regions = ref<HexRegion[]>([]);
const structures = ref<Reply | null>(null);
const app = ref<Reply | null>(null);
const usage = ref<Reply | null>(null);
let stopPolling = () => {};

async function refresh() {
  const r = await quiet({ cmd: "regions" });
  if (r.ok) assignIfChanged(regions, r.regions);
  const i = await quiet({ cmd: "inspect" });
  if (i.ok) {
    assignIfChanged(structures, i.structures);
    assignIfChanged(app, i.app ?? null);
  }
  const u = await quiet({ cmd: "usage" });
  assignIfChanged(usage, u.ok ? u : null);
}
onMounted(() => {
  stopPolling = poll(refresh, 800);
});
onBeforeUnmount(() => stopPolling());
watch(() => state.revision, refresh);

const percent = (used: number, total: number) => (total > 0 ? Math.max(0, Math.min(100, Math.round((used / total) * 100))) : 0);
const heap = computed(() => {
  const u = usage.value;
  if (!u) return null;
  const size = u.heap.end - u.heap.start;
  const used = u.heap_high_water ? u.heap_high_water - u.heap.start : 0;
  return { used, size, percent: percent(used, size), mark: u.heap_break };
});
const stack = computed(() => {
  const u = usage.value;
  if (!u) return null;
  const size = u.stack.end - u.stack.start;
  const used = u.stack_low_water == null ? 0 : u.stack.end - u.stack_low_water;
  return { used, size, percent: percent(used, size), mark: u.sp };
});

// Enregistrer l'état (.nds) : on choisit ce qu'il contient ; tout est coché par défaut.
const paused = computed(() => !state.running && state.loaded);
async function saveState() {
  const chosen = await choiceDialog(
    t("state.choose"),
    [
      { id: "app", label: t("state.parts.app"), hint: t("state.hints.app") },
      { id: "memory", label: t("state.parts.memory"), hint: t("state.hints.memory") },
      { id: "fs", label: t("state.parts.fs"), hint: t("state.hints.fs") },
      { id: "state", label: t("state.parts.state"), hint: t("state.hints.state") },
    ],
    { title: t("state.save"), accept: t("state.save") },
  );
  if (!chosen) return;
  const document = await emulator.dump(chosen);
  if (document) download(`${stem(state.appName || "nwasabi")}.nds`, JSON.stringify(document), "application/json");
}
</script>

<template>
  <div class="memory">
    <section class="card state">
      <div>
        <h2>{{ t("state.title") }}</h2>
        <p class="muted">{{ t("state.help") }}</p>
      </div>
      <button :disabled="!paused" :title="paused ? t('state.saveHint') : t('common.pauseRequired')" @click="saveState"><Icon name="floppy" :size="18" /> {{ t("state.save") }}</button>
    </section>

    <section class="card state">
      <div>
        <h2>{{ t("quick.title") }}</h2>
        <p class="muted">{{ t("quick.help") }}</p>
        <p v-if="quick.savedAt" class="muted">{{ t("quick.last", { app: quick.appName || "—", when: new Date(quick.savedAt).toLocaleString(locale) }) }}</p>
      </div>
      <div class="quick-buttons">
        <button :disabled="!state.loaded" :title="t('quick.saveHint')" @click="quickSave(emulator, { saved: t('quick.saved'), failed: t('quick.failed') })">{{ t("quick.save") }}</button>
        <button :disabled="!quick.savedAt" :title="t('quick.loadHint')" @click="quickLoad(emulator, { loaded: t('quick.loaded'), none: t('quick.none'), failed: t('quick.failed') })">{{ t("quick.load") }}</button>
      </div>
    </section>

    <section class="card">
      <h2>{{ t("memory.usage") }}</h2>
      <div v-if="heap && stack" class="usage">
        <div :title="t('memory.heapHint')">
          <div class="label"><span>{{ t("memory.heap") }}</span><span class="mono">{{ formatBytes(heap.used, locale) }} / {{ formatBytes(heap.size, locale) }} · {{ heap.percent }} %</span></div>
          <div class="bar" role="progressbar" :aria-valuenow="heap.percent" aria-valuemin="0" aria-valuemax="100" :aria-label="t('memory.heap')"><div :style="{ width: heap.percent + '%' }" /></div>
          <small class="muted mono">{{ t("memory.heapBreak", { address: hex0x(heap.mark) }) }}</small>
        </div>
        <div :title="t('memory.stackHint')">
          <div class="label"><span>{{ t("memory.stack") }}</span><span class="mono">{{ formatBytes(stack.used, locale) }} / {{ formatBytes(stack.size, locale) }} · {{ stack.percent }} %</span></div>
          <div class="bar stack" role="progressbar" :aria-valuenow="stack.percent" aria-valuemin="0" aria-valuemax="100" :aria-label="t('memory.stack')"><div :style="{ width: stack.percent + '%' }" /></div>
          <small class="muted mono">{{ t("memory.sp", { address: hex0x(stack.mark) }) }}</small>
        </div>
      </div>
      <p v-else class="muted">{{ t("memory.noApp") }}</p>
    </section>

    <section class="card">
      <h2>{{ t("memory.map") }}</h2>
      <MemoryMap :regions="regions" :structures="structures" :app="app" :usage="usage" />
    </section>

    <section class="card">
      <h2>{{ t("bookmarks.title") }}</h2>
      <BookmarksCard />
    </section>

    <section class="card">
      <h2>{{ t("memory.hex") }}</h2>
      <HexEditor :regions="regions" />
    </section>

    <section class="card">
      <h2>{{ t("scan.title") }}</h2>
      <MemoryScanner :regions="regions" />
    </section>

    <section class="card">
      <h2>{{ t("watch.title") }}</h2>
      <p class="muted help">{{ t("watch.help") }}</p>
      <WatchPoints />
    </section>
  </div>
</template>

<style scoped>
.memory { display: grid; gap: 20px; }
.card:first-child { border-top: none; padding-top: 0; }
.card { border-top: 1px solid var(--line); padding: 16px 0 0; }
.card > .help { margin: 4px 0 10px; font-size: 12px; }
.state { display: flex; gap: 12px; align-items: center; justify-content: space-between; }
.state p { margin: 0; font-size: 12px; }
.state p + p { margin-top: 4px; }
.quick-buttons { display: grid; gap: 6px; }
.usage { display: grid; gap: 10px; }
.label { display: flex; justify-content: space-between; gap: 8px; font-size: 13px; }
.bar { height: 10px; background: var(--surface-2); border: 1px solid var(--line); border-radius: 6px; overflow: hidden; margin: 3px 0; }
.bar div { height: 100%; background: var(--accent); transition: width 0.3s; }
.bar.stack div { background: #e3a008; }
</style>
