<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { Reply } from "@nwasabi/js";
import { showInMemory, ui } from "../../ui.js";
import { hex0x } from "../../util.js";
import { poll } from "../../poll.js";
import AddressLink from "../AddressLink.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;
const info = ref<Reply | null>(null);
const selected = ref("slot_info");
const filter = ref("");
let stopPolling = () => {};

async function refresh() {
  const r = await quiet({ cmd: "inspect" });
  if (r.ok) info.value = r;
}
onMounted(() => {
  stopPolling = poll(refresh, 1000);
});
onBeforeUnmount(() => stopPolling());
watch(() => state.revision, refresh);
watch(
  () => ui.structureGoto,
  (goto) => goto && (selected.value = goto.section),
  { immediate: true },
);

const SECTIONS = ["slot_info", "slot_header", "kernel_header", "userland_header", "vector_table", "storage", "device_name", "apps", "app"];
const ADDRESS_KEY = /address|base|start|end|header|entry|stack|draw_string|flash|ram|heap|bss|data$/;

interface Field {
  name: string;
  value: any; // JSON hétérogène relu depuis la mémoire
  kind: "range" | "string" | "address" | "number" | "boolean" | "null";
}

/** Les champs de la section choisie, à plat : { name, value, kind }. */
const fields = computed((): Field[] => {
  const data = info.value;
  if (!data) return [];
  const raw = selected.value === "app" ? data.app : data.structures[selected.value];
  if (raw === undefined || raw === null) return [];
  if (selected.value === "device_name") return [{ name: "device_name", value: raw, kind: "string" }];
  const out: Field[] = [];
  const entries: Array<[string, unknown]> = Array.isArray(raw) ? raw.map((v, i): [string, unknown] => [String(i), v]) : Object.entries(raw);
  for (const [name, value] of entries) {
    if (value && typeof value === "object" && "start" in value && "end" in value) out.push({ name, value, kind: "range" });
    else if (value && typeof value === "object" && !Array.isArray(value)) out.push({ name, value: Object.entries(value).map(([k, v]) => `${k}=${typeof v === "number" ? hex0x(v) : v}`).join("  "), kind: "string" });
    else if (Array.isArray(value)) out.push({ name, value: value.join("; ") || "—", kind: "string" });
    else if (typeof value === "number") out.push({ name, value, kind: ADDRESS_KEY.test(name) ? "address" : "number" });
    else if (typeof value === "boolean") out.push({ name, value, kind: "boolean" });
    else out.push({ name, value: value ?? null, kind: value === null ? "null" : "string" });
  }
  return out;
});

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return q ? fields.value.filter((f) => f.name.toLowerCase().includes(q) || String(f.value).toLowerCase().includes(q)) : fields.value;
});

const present = (id: string) => {
  const d = info.value;
  if (!d) return false;
  if (id === "app") return !!d.app;
  return d.structures[id] !== undefined && d.structures[id] !== null;
};
const invalid = (id: string) => info.value?.structures[id]?.valid === false;
const appsRows = computed(() => (selected.value === "apps" ? info.value?.structures.apps ?? [] : []));

async function copy(text: unknown) {
  try {
    await navigator.clipboard.writeText(String(text));
  } catch {
    /* le presse-papiers peut être refusé */
  }
}
const textOf = (f: Field) => (f.kind === "range" ? `${hex0x(f.value.start)}-${hex0x(f.value.end)}` : f.kind === "address" ? hex0x(f.value) : String(f.value));
</script>

<template>
  <div v-if="info" class="structures">
    <nav class="tree" role="tree" :aria-label="t('structures.tree')">
      <p class="tree-title">{{ t("structures.explorer") }}</p>
      <button
        v-for="id in SECTIONS"
        :key="id"
        role="treeitem"
        class="node"
        :class="{ active: selected === id, missing: !present(id) }"
        :aria-selected="selected === id"
        @click="selected = id"
      >
        <span class="twist" aria-hidden="true">●</span>
        <span class="mono">{{ t(`structures.sections.${id}`) }}</span>
        <span v-if="invalid(id)" class="chip bad" :title="t('structures.invalid')">!</span>
      </button>
    </nav>

    <div class="detail">
      <header>
        <h2 class="mono">{{ t(`structures.sections.${selected}`) }}</h2>
        <input v-model="filter" type="search" class="mono" :placeholder="t('structures.filter')" :aria-label="t('structures.filter')" data-keep-keys />
      </header>
      <p class="muted desc">{{ t(`structures.descriptions.${selected}`) }}</p>
      <p v-if="invalid(selected)" class="error">{{ t("structures.invalid") }}</p>

      <table v-if="appsRows.length" class="mono code">
        <thead><tr><th>{{ t("structures.name") }}</th><th>{{ t("structures.address") }}</th><th>API</th><th>{{ t("structures.size") }}</th></tr></thead>
        <tbody>
          <tr v-for="a in appsRows" :key="a.address">
            <td class="str">"{{ a.name }}"</td>
            <td><AddressLink :address="a.address" /></td>
            <td class="num">{{ a.api_level }}</td><td class="num">{{ a.size }}</td>
          </tr>
        </tbody>
      </table>
      <p v-else-if="selected === 'apps'" class="muted">{{ t("structures.noApps") }}</p>

      <table v-else-if="shown.length" class="mono code">
        <tbody>
          <tr v-for="f in shown" :key="f.name" @dblclick="copy(textOf(f))" :title="t('structures.copyHint')">
            <td class="key">{{ f.name }}</td>
            <td class="eq" aria-hidden="true">:</td>
            <td v-if="f.kind === 'address'" class="num"><AddressLink :address="f.value" /> <span class="muted">({{ f.value }})</span></td>
            <td v-else-if="f.kind === 'range'" class="num">
              <AddressLink :address="f.value.start" :length="f.value.end - f.value.start" /> – {{ hex0x(f.value.end) }}
              <span class="muted">({{ f.value.end - f.value.start }} {{ t("structures.bytes") }})</span>
            </td>
            <td v-else-if="f.kind === 'number'" class="num">{{ f.value }} <span class="muted">({{ hex0x(f.value, 1) }})</span></td>
            <td v-else-if="f.kind === 'boolean'" :class="f.value ? 'yes' : 'no'">{{ f.value }}</td>
            <td v-else-if="f.kind === 'null'" class="muted">null</td>
            <td v-else class="str">"{{ f.value }}"</td>
            <td class="copy"><button class="icon" :aria-label="t('structures.copy')" :title="t('structures.copy')" @click="copy(textOf(f))">⧉</button></td>
          </tr>
        </tbody>
      </table>
      <p v-else class="muted">{{ present(selected) ? t("structures.noMatch") : t("structures.notFound") }}</p>
    </div>
  </div>
</template>

<style scoped>
.structures { display: grid; grid-template-columns: 190px 1fr; gap: 0; height: 100%; min-height: 420px; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.tree { background: var(--surface-2); border-right: 1px solid var(--line); padding: 6px 0; overflow: auto; }
.tree-title { margin: 2px 10px 6px; font-size: 11px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.node { width: 100%; border: none; border-radius: 0; background: transparent; justify-content: flex-start; padding: 3px 10px; font-size: 12.5px; }
.node:hover { background: var(--accent-soft); }
.node.active { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent); font-weight: 600; }
.node.missing { opacity: 0.5; }
.twist { color: var(--muted); font-size: 7px; width: 10px; text-align: center; }
.node.active .twist { color: var(--accent-strong); }
.chip.bad { background: color-mix(in srgb, var(--danger) 18%, transparent); color: var(--danger); }
.detail { padding: 10px 14px; overflow: auto; min-width: 0; background: var(--surface); }
header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
header h2 { margin: 0; text-transform: none; font-size: 14px; color: var(--text); }
header input { width: 11em; }
.desc { font-size: 12px; margin: 4px 0 10px; }
.code { border-spacing: 0; }
.code tr:hover td { background: var(--accent-soft); }
.code td { padding: 2px 6px; }
.key { color: var(--syn-key); }
.eq { color: var(--muted); width: 1ch; padding: 0; }
.num { color: var(--syn-num); }
.str { color: var(--syn-str); }
.yes { color: #1a7f37; font-weight: 600; }
.no { color: var(--danger); font-weight: 600; }
.link { border: none; background: none; padding: 0; color: inherit; text-decoration: underline dotted; font: inherit; cursor: pointer; }
.link:hover { color: var(--accent-strong); }
.copy { text-align: right; opacity: 0; }
tr:hover .copy, .copy:focus-within { opacity: 1; }
@media (max-width: 560px) { .structures { grid-template-columns: 1fr; } .tree { border-right: none; border-bottom: 1px solid var(--line); } }
</style>
