<script setup lang="ts">
// Les adresses que la recherche a retenues. Quand il y en a peu, leurs valeurs sont relues en continu.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { formatValue } from "../scan.js";
import type { ScanType } from "../scan.js";
import { addWatched, memoryAccess, scan, store } from "../scanStore.js";
import { hex0x, poll } from "../util.js";
import AddressLink from "./AddressLink.vue";
import Icon from "./Icon.vue";

const { t } = useI18n();
const emulator = useEmu();
const { readValue } = memoryAccess(emulator);

const ROWS = 100; // lignes affichées
const live = ref<number[]>([]);
const count = computed(() => (void store.revision, scan.value.count));
const rows = computed(() => {
  void store.revision;
  const s = scan.value;
  return Array.from({ length: Math.min(ROWS, s.count) }, (_, i) => ({
    address: s.addresses[i] as number,
    value: live.value[i] ?? (s.values[i] as number),
    previous: s.previous[i],
  }));
});
const shown = (value: number | undefined, type: ScanType) => (value === undefined ? "" : formatValue(value, type));

let stopPolling = () => {};
onMounted(() => {
  stopPolling = poll(async () => {
    if (!emulator.state.loaded || !store.started) return;
    if (scan.value.count > ROWS) return void (live.value = []);
    const values: number[] = [];
    for (const address of scan.value.addresses) values.push((await readValue(address, scan.value.type)) ?? Number.NaN);
    live.value = values;
  }, 250);
});
onBeforeUnmount(() => stopPolling());
</script>

<template>
  <div class="gh">
    <div class="gh-head cols" aria-hidden="true"><span>{{ t("scan.colAddress") }}</span><span>{{ t("scan.colValue") }}</span><span>{{ t("scan.colPrevious") }}</span><span /></div>
    <div class="gh-rows">
      <div v-for="row in rows" :key="row.address" class="gh-row cols" @dblclick="addWatched(row.address, scan.type, row.value)">
        <AddressLink :address="row.address" />
        <span class="mono">{{ shown(row.value, scan.type) }}</span>
        <span class="mono muted">{{ shown(row.previous, scan.type) }}</span>
        <button class="flat icon" :title="t('scan.add')" :aria-label="t('scan.add')" @click="addWatched(row.address, scan.type, row.value)"><Icon name="plus" :size="14" /></button>
      </div>
      <p v-if="!rows.length" class="empty muted">{{ t("scan.none") }}</p>
    </div>
    <div v-if="count > ROWS" class="gh-status"><span class="muted">{{ t("scan.showing", { n: ROWS }) }}</span></div>
  </div>
</template>

<style scoped>
.cols { grid-template-columns: 120px 1fr 1fr 28px; }
.link { border: none; background: none; padding: 0; text-align: left; color: var(--accent-strong); cursor: pointer; }
.link:hover { text-decoration: underline; }
.empty { padding: 14px; text-align: center; margin: 0; }
.gh-rows { max-height: 260px; overflow: auto; }
</style>
