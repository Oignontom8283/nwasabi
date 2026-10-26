<script setup>
// Représentation horizontale de la mémoire : les zones côte à côte, ce qu'elles contiennent, les repères des
// structures principales (qui renvoient à l'onglet « Structures »), et ce qui est réel ou simulé.
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { showInMemory, showStructure } from "../ui.js";
import { formatBytes, hex0x } from "../util.js";

const props = defineProps({
  regions: { type: Array, required: true },
  structures: { type: Object, default: null },
  app: { type: Object, default: null },
  usage: { type: Object, default: null },
});
const { t, locale } = useI18n();

// Largeur d'une zone : logarithmique, pour que la petite RAM du noyau reste visible à côté des 8 Mio de flash.
const blocks = computed(() => {
  const sorted = [...props.regions].sort((a, b) => a.base - b.base);
  const weights = sorted.map((r) => Math.max(6, Math.log2(r.size) - 9));
  const total = weights.reduce((a, b) => a + b, 0);
  return sorted.map((r, i) => ({ ...r, width: (weights[i] / total) * 100 }));
});

// Les segments connus, avec leur nature : "real" (ce que voit la calculatrice : code de l'application, sa RAM, ses fichiers)
// ou "simulated" (fabriqué par l'émulateur : en-têtes et tables du système, RAM du noyau, mémoires du système).
function segments() {
  const out = [];
  const s = props.structures;
  const add = (start, end, kind, label, extra = {}) => {
    if (typeof start === "number" && typeof end === "number" && end > start) out.push({ start, end, kind, label, ...extra });
  };
  if (s) {
    add(s.slot_info?.address, s.slot_info?.address + 16, "simulated", "SlotInfo", { cls: "system" });
    add(s.slot_header?.address, s.userland_header?.address ? s.userland_header.address + 0x1000 : undefined, "simulated", t("memmap.systemHeaders"), { cls: "system" });
    add(s.userland_header?.external_apps_flash?.start, s.userland_header?.external_apps_flash?.end, "real", t("memmap.appsFlash"), { cls: "appsflash", faded: true });
    add(s.storage?.address, s.storage?.address + s.storage?.size, "real", t("memmap.files"), { cls: "files" });
  }
  if (props.app) {
    add(props.app.flash_start, props.app.flash_start + props.app.flash_size, "real", t("memmap.appCode"), { cls: "app" });
  }
  if (props.usage) {
    const ramStart = s?.userland_header?.external_apps_ram?.start ?? props.usage.heap.start;
    add(ramStart, props.usage.heap.start, "real", t("memmap.appData"), { cls: "data" });
    add(props.usage.heap.start, props.usage.heap.end, "real", t("memmap.heap"), { cls: "heap" });
    add(props.usage.stack.start, props.usage.stack.end, "real", t("memmap.stack"), { cls: "stack" });
  }
  return out;
}

const markers = computed(() => {
  const s = props.structures;
  if (!s) return [];
  const list = [
    ["slot_info", "SlotInfo", s.slot_info?.address],
    ["kernel_header", "KernelHeader", s.kernel_header?.address],
    ["userland_header", "UserlandHeader", s.userland_header?.address],
    ["vector_table", t("memmap.vectors"), s.vector_table?.address],
    ["storage", t("memmap.files"), s.storage?.address],
  ];
  return list.filter(([, , address]) => typeof address === "number").map(([section, label, address]) => ({ section, label, address }));
});

function layout(block) {
  const inside = segments().filter((x) => x.end > block.base && x.start < block.base + block.size);
  return inside.map((x) => {
    const start = Math.max(x.start, block.base);
    const end = Math.min(x.end, block.base + block.size);
    return { ...x, left: ((start - block.base) / block.size) * 100, width: Math.max(((end - start) / block.size) * 100, 0.6), from: start, to: end };
  });
}

function markerPosition(marker) {
  const block = blocks.value.find((b) => marker.address >= b.base && marker.address < b.base + b.size);
  if (!block) return null;
  const before = blocks.value.slice(0, blocks.value.indexOf(block)).reduce((sum, b) => sum + b.width, 0);
  return before + ((marker.address - block.base) / block.size) * block.width;
}

// Les repères proches se chevaucheraient : on les étage sur trois lignes (chacun dans la première où il tient).
const placedMarkers = computed(() => {
  const list = markers.value.map((m) => ({ ...m, left: markerPosition(m) })).filter((m) => m.left !== null).sort((a, b) => a.left - b.left);
  const ends = [-Infinity, -Infinity, -Infinity];
  return list.map((m) => {
    const width = m.label.length * 1.5 + 3; // largeur approximative, en % de la carte
    let row = ends.findIndex((end) => m.left - width / 2 > end);
    if (row < 0) row = ends.indexOf(Math.min(...ends));
    ends[row] = m.left + width / 2;
    return { ...m, row };
  });
});

const tip = (seg) => `${seg.label}\n${hex0x(seg.from)} → ${hex0x(seg.to)} (${formatBytes(seg.to - seg.from, locale.value)})`;
</script>

<template>
  <div class="map" role="group" :aria-label="t('memmap.label')">
    <div class="markers" aria-label="markers">
      <button v-for="m in placedMarkers" :key="m.section" class="marker" :style="{ left: m.left + '%', top: m.row * 22 + 'px', '--drop': (2 - m.row) * 22 + 'px' }" :title="t('memmap.markerHint', { name: m.label, address: hex0x(m.address) })" @click="showStructure(m.section)">
        {{ m.label }}
      </button>
    </div>

    <p class="title">{{ t("memmap.contents") }}</p>
    <div class="strip">
      <div v-for="b in blocks" :key="b.name" class="block" :class="b.kind" :style="{ width: b.width + '%' }">
        <button
          v-for="seg in layout(b)"
          :key="seg.label + seg.from"
          class="seg"
          :class="[seg.cls, { faded: seg.faded }]"
          :style="{ left: seg.left + '%', width: seg.width + '%' }"
          :title="tip(seg)"
          :aria-label="tip(seg)"
          @click="showInMemory(seg.from)"
        />
      </div>
    </div>
    <div class="strip names">
      <div v-for="b in blocks" :key="b.name" class="block-name" :style="{ width: b.width + '%' }" :title="`${hex0x(b.base)} · ${formatBytes(b.size, locale)}`">
        <span>{{ b.name }}</span>
        <small class="muted mono">{{ hex0x(b.base) }}</small>
      </div>
    </div>

    <p class="title">{{ t("memmap.realOrSimulated") }}</p>
    <div class="strip">
      <div v-for="b in blocks" :key="b.name" class="block" :class="b.kind" :style="{ width: b.width + '%' }">
        <button
          v-for="seg in layout(b).filter((x) => !x.faded)"
          :key="seg.label + seg.from"
          class="seg"
          :class="seg.kind"
          :style="{ left: seg.left + '%', width: seg.width + '%' }"
          :title="tip(seg)"
          :aria-label="tip(seg)"
          @click="showInMemory(seg.from)"
        />
        <span v-if="b.kind === 'reserved' || b.name.includes('itcm') || b.name.includes('internal')" class="seg simulated whole" :title="t('memmap.simulatedWhole', { name: b.name })" />
      </div>
    </div>

    <ul class="legend">
      <li><i class="sw app" /> {{ t("memmap.appCode") }}</li>
      <li><i class="sw data" /> {{ t("memmap.appData") }}</li>
      <li><i class="sw heap" /> {{ t("memmap.heap") }}</li>
      <li><i class="sw stack" /> {{ t("memmap.stack") }}</li>
      <li><i class="sw files" /> {{ t("memmap.files") }}</li>
      <li><i class="sw system" /> {{ t("memmap.systemHeaders") }}</li>
      <li><i class="sw real" /> {{ t("memmap.real") }}</li>
      <li><i class="sw simulated" /> {{ t("memmap.simulated") }}</li>
    </ul>
    <p class="muted note">{{ t("memmap.note") }}</p>
  </div>
</template>

<style scoped>
.map { display: grid; gap: 4px; }
.markers { position: relative; height: 72px; }
.marker {
  position: absolute; top: 0; transform: translateX(-50%); padding: 0 6px; font-size: 11px; border-radius: 6px; background: var(--surface);
}
.marker::after { content: ""; position: absolute; left: 50%; top: 100%; width: 1px; height: calc(var(--drop, 0px) + 4px); background: var(--muted); }
.title { margin: 6px 0 0; font-size: 12px; color: var(--muted); }
.strip { display: flex; gap: 3px; }
.block { position: relative; height: 26px; background: var(--surface-2); border: 1px solid var(--line); border-radius: 5px; overflow: hidden; flex: none; }
.block.reserved { background: repeating-linear-gradient(45deg, var(--surface-2), var(--surface-2) 4px, var(--line) 4px, var(--line) 8px); }
.seg { position: absolute; top: 0; bottom: 0; min-width: 2px; border: none; border-radius: 0; padding: 0; opacity: 0.95; }
.seg.faded { opacity: 0.28; }
.seg.app { background: #3f7a1f; } .seg.data { background: #6e9f2a; } .seg.heap { background: #b5dd78; } .seg.stack { background: #e3a008; }
.seg.files { background: #2b6cb0; } .seg.system { background: #7b8794; } .seg.appsflash { background: #3f7a1f; }
.seg.real { background: var(--accent); } .seg.simulated { background: repeating-linear-gradient(45deg, #7b8794, #7b8794 3px, #a8b2bd 3px, #a8b2bd 6px); }
.seg.whole { left: 0; width: 100%; }
.names .block-name { display: grid; font-size: 11px; line-height: 1.2; overflow: hidden; flex: none; }
.names .block-name span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.legend { list-style: none; margin: 4px 0 0; padding: 0; display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 12px; }
.sw { display: inline-block; width: 12px; height: 12px; border-radius: 3px; vertical-align: -2px; }
.sw.app { background: #3f7a1f; } .sw.data { background: #6e9f2a; } .sw.heap { background: #b5dd78; } .sw.stack { background: #e3a008; }
.sw.files { background: #2b6cb0; } .sw.system { background: #7b8794; } .sw.real { background: var(--accent); }
.sw.simulated { background: repeating-linear-gradient(45deg, #7b8794, #7b8794 3px, #a8b2bd 3px, #a8b2bd 6px); }
.note { margin: 2px 0 0; font-size: 12px; }
</style>
