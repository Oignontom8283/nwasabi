<script setup lang="ts">
// Les réglages de la recherche et son déroulement. Les résultats et la liste sont des composants à part (ils relisent la
// mémoire plusieurs fois par seconde) : ainsi les menus de cette partie ne sont jamais redessinés par leurs relevés.
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { NEEDS_PREVIOUS, NEEDS_VALUE, SCAN_TYPES, Scan, parseValue } from "../scan.js";
import type { Compare, Criteria, ScanType } from "../scan.js";
import { memoryAccess, scan, store } from "../scanStore.js";
import { hex0x } from "../util.js";
import type { HexRegion } from "./HexEditor.vue";
import ScanResults from "./ScanResults.vue";
import ScanWatchList from "./ScanWatchList.vue";

const props = defineProps<{ regions: HexRegion[] }>();
const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;
const { read } = memoryAccess(emulator);

const type = ref<ScanType>("u32");
const compare = ref<Compare>("exact");
const first = ref("");
const second = ref("");
const zone = ref("ram");
const aligned = ref(true);
const message = ref("");

const COMPARES: Compare[] = ["exact", "greater", "less", "between", "changed", "unchanged", "increased", "decreased", "unknown"];
/** Avant la première recherche on ne compare qu'à une valeur (ou on garde tout) ; ensuite « inconnue » n'a plus de sens. */
const available = computed(() => COMPARES.filter((c) => (store.started ? c !== "unknown" : !NEEDS_PREVIOUS.has(c))));
watch(
  () => store.started,
  () => {
    if (!available.value.includes(compare.value)) compare.value = "exact";
  },
);

const readable = computed(() => props.regions.filter((r) => r.kind !== "reserved"));
const zones = computed(() => (zone.value === "ram" ? readable.value.filter((r) => r.kind === "ram") : zone.value === "all" ? readable.value : readable.value.filter((r) => r.name === zone.value)));

function criteria(): Criteria | null {
  const c: Criteria = { compare: compare.value, type: store.started ? scan.value.type : type.value };
  if (NEEDS_VALUE.has(compare.value)) {
    const a = parseValue(first.value, c.type);
    if (a === null) {
      message.value = t("scan.badValue", { text: first.value, type: t(`scan.types.${c.type}`) });
      return null;
    }
    c.a = a;
    if (compare.value === "between") {
      const b = parseValue(second.value, c.type);
      if (b === null) {
        message.value = t("scan.badValue", { text: second.value, type: t(`scan.types.${c.type}`) });
        return null;
      }
      c.b = b;
    }
  }
  return c;
}

async function run() {
  message.value = "";
  const wanted = criteria();
  if (!wanted || store.busy) return;
  store.busy = true;
  try {
    if (store.started) await scan.value.next(read, wanted);
    else {
      await scan.value.first(zones.value, read, wanted, aligned.value);
      store.started = true;
    }
    store.revision++;
    if (scan.value.truncated) message.value = t("scan.truncated", { n: scan.value.count });
  } finally {
    store.busy = false;
  }
}

function reset() {
  scan.value = new Scan();
  store.started = false;
  store.revision++;
  message.value = "";
}
// Une autre application : les adresses trouvées ne veulent plus rien dire.
watch(() => state.appName, reset);

const inputs = computed(() => ({ one: NEEDS_VALUE.has(compare.value), two: compare.value === "between" }));
const locked = computed(() => store.started);
</script>

<template>
  <div class="scanner">
    <p class="muted help">{{ t("scan.help") }}</p>
    <p v-if="!state.loaded" class="muted">{{ t("scan.noApp") }}</p>

    <div class="controls">
      <label class="field">{{ t("scan.type") }}
        <select v-model="type" :disabled="locked" v-memo="[type, locked]">
          <option v-for="k in SCAN_TYPES" :key="k" :value="k">{{ t(`scan.types.${k}`) }}</option>
        </select>
      </label>
      <label class="field">{{ t("scan.zone") }}
        <select v-model="zone" :disabled="locked" v-memo="[zone, locked, readable.length]">
          <option value="ram">{{ t("scan.zoneRam") }}</option>
          <option value="all">{{ t("scan.zoneAll") }}</option>
          <option v-for="r in readable" :key="r.name" :value="r.name">{{ r.name }} · {{ hex0x(r.base) }}</option>
        </select>
      </label>
      <label class="field">{{ t("scan.compare") }}
        <select v-model="compare" v-memo="[compare, store.started]">
          <option v-for="c in available" :key="c" :value="c">{{ t(`scan.compares.${c}`) }}</option>
        </select>
      </label>
      <label v-if="inputs.one" class="field">{{ compare === "between" ? t("scan.min") : t("scan.value") }}
        <input v-model="first" class="mono" data-keep-keys :aria-label="t('scan.value')" @keydown.enter="run" />
      </label>
      <label v-if="inputs.two" class="field">{{ t("scan.max") }}
        <input v-model="second" class="mono" data-keep-keys :aria-label="t('scan.max')" @keydown.enter="run" />
      </label>
    </div>
    <label v-if="!store.started" class="inline"><input v-model="aligned" type="checkbox" /> {{ t("scan.aligned") }}</label>

    <div class="actions">
      <button class="primary" :disabled="!state.loaded || store.busy" @click="run">{{ store.busy ? t("scan.scanning") : store.started ? t("scan.next") : t("scan.first") }}</button>
      <button :disabled="!store.started || store.busy" @click="reset">{{ t("scan.reset") }}</button>
      <span v-if="store.started" class="muted">{{ t("scan.results", { n: scan.count }, scan.count) }}</span>
    </div>
    <p v-if="message" class="error" role="alert">{{ message }}</p>

    <ScanResults v-if="store.started" />

    <h3>{{ t("scan.listTitle") }}</h3>
    <p class="muted help">{{ t("scan.listHelp") }}</p>
    <ScanWatchList />
  </div>
</template>

<style scoped>
.scanner { display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; }
.help { margin: 0; font-size: 12px; }
.controls { display: flex; flex-wrap: wrap; gap: 10px; align-items: end; }
.inline { display: inline-flex; gap: 6px; align-items: center; font-size: 12px; }
.actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
h3 { margin: 6px 0 -4px; }
</style>
