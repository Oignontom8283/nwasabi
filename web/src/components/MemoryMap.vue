<script setup lang="ts">
// Représentation horizontale de la mémoire : les zones côte à côte, ce qu'elles contiennent, les repères des
// structures principales (qui renvoient à l'onglet « Structures »), et ce qui est réel ou simulé.
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { showInMemory, showStructure } from "../ui.js";
import { formatBytes, hex0x } from "../util.js";

// Les données viennent des réponses JSON du module (`regions`, `inspect`, `usage`) : leurs champs ne sont pas typés.
type Json = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
const props = defineProps<{ regions: Json[]; structures: Json | null; app: Json | null; usage: Json | null }>();
const { t, locale } = useI18n();

interface Region {
  name: string;
  base: number;
  size: number;
  kind: string;
}
interface Block extends Region {
  /** Largeur de la zone sur la carte, en % de la largeur totale. */
  width: number;
}
interface Segment {
  start: number;
  end: number;
  /** "real" (comme sur la calculatrice) ou "simulated" (fabriqué par l'émulateur). */
  kind: "real" | "simulated";
  label: string;
  cls: string;
  faded?: boolean;
}
interface Placed extends Segment {
  left: number;
  width: number;
  from: number;
  to: number;
}
interface Marker {
  section: string;
  label: string;
  address: number;
  /** Position sur la carte, en % de la largeur. */
  left: number;
  /** Fin de la plage sur la carte (en %), pour une structure qui a une étendue ; `null` pour un simple repère. */
  end: number | null;
  /** Taille de la plage, en octets. */
  size: number | null;
}
interface Entry {
  section: string;
  label: string;
  /** Adresse, ou plage `début → fin · taille`. */
  where: string;
}
interface Tip {
  title: string;
  kind: "real" | "simulated" | null;
  lines: string[];
  x: number;
  y: number;
}

// Largeur d'une zone : logarithmique, pour que la petite RAM du noyau reste visible à côté des 8 Mio de flash.
const blocks = computed((): Block[] => {
  const sorted = [...(props.regions as Region[])].sort((a, b) => a.base - b.base);
  const weights = sorted.map((r) => Math.max(6, Math.log2(r.size) - 9));
  const total = weights.reduce((a, b) => a + b, 0);
  return sorted.map((r, i) => ({ ...r, width: ((weights[i] ?? 0) / total) * 100 }));
});

// Les segments connus, avec leur nature : "real" (ce que voit la calculatrice : code de l'application, sa RAM, ses fichiers)
// ou "simulated" (fabriqué par l'émulateur : en-têtes et tables du système, RAM du noyau, mémoires du système).
function segments(): Segment[] {
  const out: Segment[] = [];
  const s = props.structures;
  const add = (start: unknown, end: unknown, kind: Segment["kind"], label: string, cls: string, faded = false) => {
    if (typeof start === "number" && typeof end === "number" && end > start) out.push({ start, end, kind, label, cls, faded });
  };
  if (s) {
    add(s.slot_info?.address, s.slot_info?.address + 16, "simulated", "SlotInfo", "system");
    add(s.slot_header?.address, s.userland_header?.address ? s.userland_header.address + 0x1000 : undefined, "simulated", t("memmap.systemHeaders"), "system");
    add(s.userland_header?.external_apps_flash?.start, s.userland_header?.external_apps_flash?.end, "real", t("memmap.appsFlash"), "appsflash", true);
    add(s.storage?.address, s.storage?.address + s.storage?.size, "real", t("memmap.files"), "files");
  }
  if (props.app) {
    add(props.app.flash_start, props.app.flash_start + props.app.flash_size, "real", t("memmap.appCode"), "app");
  }
  if (props.usage) {
    const ramStart = s?.userland_header?.external_apps_ram?.start ?? props.usage.heap.start;
    add(ramStart, props.usage.heap.start, "real", t("memmap.appData"), "data");
    add(props.usage.heap.start, props.usage.heap.end, "real", t("memmap.heap"), "heap");
    add(props.usage.stack.start, props.usage.stack.end, "real", t("memmap.stack"), "stack");
  }
  return out;
}

const markers = computed(() => {
  const s = props.structures;
  if (!s) return [];
  const list: Array<[string, string, unknown]> = [
    ["slot_info", "SlotInfo", s.slot_info?.address],
    ["kernel_header", "KernelHeader", s.kernel_header?.address],
    ["userland_header", "UserlandHeader", s.userland_header?.address],
    ["vector_table", t("memmap.vectors"), s.vector_table?.address],
    ["storage", t("memmap.files"), s.storage?.address],
  ];
  return list.flatMap(([section, label, address]) => (typeof address === "number" ? [{ section, label, address }] : []));
});

// Les repères au-dessus de la carte : un trait par structure (une accolade quand elle a une étendue, comme les fichiers) ;
// ceux qui se toucheraient sont fusionnés en un seul libellé.
function positionOf(address: number): number | null {
  const index = blocks.value.findIndex((b) => address >= b.base && address < b.base + b.size);
  const block = blocks.value[index];
  if (!block) return null;
  const before = blocks.value.slice(0, index).reduce((sum, b) => sum + b.width, 0);
  return before + ((address - block.base) / block.size) * block.width;
}
const MERGE = 14; // écart minimal entre deux libellés, en % de la largeur
const align = (left: number) => (left < 12 ? "start" : left > 88 ? "end" : "mid");
function located(): Marker[] {
  const sizes: Record<string, number | undefined> = { storage: props.structures?.storage?.size };
  return markers.value.flatMap((m) => {
    const left = positionOf(m.address);
    const size = sizes[m.section];
    return left === null ? [] : [{ ...m, left, end: size ? positionOf(m.address + size - 1) : null, size: size ?? null }];
  });
}
const entryOf = (m: Marker): Entry => ({
  section: m.section,
  label: m.label,
  where: m.size ? `${hex0x(m.address)} → ${hex0x(m.address + m.size - 1)} · ${formatBytes(m.size, locale.value)}` : hex0x(m.address),
});
// Les repères ponctuels proches sont fusionnés en un seul libellé (qui déroule la liste de ce qu'il regroupe) ; les plages
// (les fichiers) ne le sont jamais : elles ont leur libellé sur une ligne à elles, relié à leur accolade par un trait.
const marks = computed(() => {
  const points = located()
    .filter((m) => m.end === null)
    .sort((a, b) => a.left - b.left);
  const clusters: Marker[][] = [];
  for (const m of points) {
    const last = clusters[clusters.length - 1];
    const previous = last?.[last.length - 1];
    if (last && previous && m.left - previous.left < MERGE) last.push(m);
    else clusters.push([m]);
  }
  return clusters.flatMap((items) => {
    const [first, second] = items;
    if (!first) return [];
    const label = !second ? first.label : items.length === 2 ? `${first.label} · ${second.label}` : `${first.label} +${items.length - 1}`;
    return [{ key: first.section, label, left: first.left, entries: items.map(entryOf), align: align(first.left) }];
  });
});
const ranges = computed(() =>
  located().flatMap((m) =>
    m.end === null ? [] : [{ key: m.section, label: m.label, left: m.left, width: Math.max(m.end - m.left, 0.5), entries: [entryOf(m)], align: align(m.left + (m.end - m.left) / 2) }],
  ),
);

function layout(block: Block, onlyReal = false): Placed[] {
  const inside = segments().filter((x) => x.end > block.base && x.start < block.base + block.size && !(onlyReal && x.faded));
  return inside.map((x) => {
    const start = Math.max(x.start, block.base);
    const end = Math.min(x.end, block.base + block.size);
    return { ...x, left: ((start - block.base) / block.size) * 100, width: Math.max(((end - start) / block.size) * 100, 0.6), from: start, to: end };
  });
}

// ---- l'infobulle qui suit la souris ----------------------------------------------------------------

const root = ref<HTMLElement | null>(null);
const tip = ref<Tip | null>(null);
function show(info: Omit<Tip, "x" | "y">, event: PointerEvent) {
  tip.value = { ...info, x: 0, y: 0 };
  move(event);
}
function move(event: PointerEvent) {
  if (!tip.value || !root.value) return;
  const box = root.value.getBoundingClientRect();
  const x = event.clientX - box.left;
  tip.value.x = Math.max(0, Math.min(x - 16, box.width - 270));
  tip.value.y = event.clientY - box.top + 18;
}
const hide = () => (tip.value = null);

const segmentTip = (seg: Placed): Omit<Tip, "x" | "y"> => ({
  title: seg.label,
  kind: seg.kind,
  lines: [`${hex0x(seg.from)} → ${hex0x(seg.to)} · ${formatBytes(seg.to - seg.from, locale.value)}`, t(`memmap.explain.${seg.cls}`), t("memmap.clickHint")],
});
const blockTip = (b: Block): Omit<Tip, "x" | "y"> => ({
  title: b.name,
  kind: b.kind === "reserved" ? "simulated" : null,
  lines: [`${hex0x(b.base)} · ${formatBytes(b.size, locale.value)}`, t(b.kind === "reserved" ? "memmap.explain.reserved" : "memmap.explain.region")],
});
</script>

<template>
  <div ref="root" class="map" role="group" :aria-label="t('memmap.label')" @pointerleave="hide">
    <p class="title">{{ t("memmap.contents") }}</p>
    <div class="marks">
      <div v-for="r in ranges" :key="r.key" class="range" :class="r.align" :style="{ left: r.left + '%', width: r.width + '%' }">
        <button class="lab" @click="showStructure(r.entries[0]?.section ?? r.key)">{{ r.label }}</button>
        <i class="rstem" />
        <i class="brace" />
        <div class="pop" role="menu">
          <button v-for="e in r.entries" :key="e.section" role="menuitem" :title="t('memmap.listHint')" @click="showStructure(e.section)"><span>{{ e.label }}</span><span class="mono muted">{{ e.where }}</span></button>
        </div>
      </div>
      <div v-for="c in marks" :key="c.key" class="mark" :class="c.align" :style="{ left: c.left + '%' }">
        <button class="lab" @click="showStructure(c.entries[0]?.section ?? c.key)">{{ c.label }}</button>
        <i class="stem" />
        <div class="pop" role="menu">
          <button v-for="e in c.entries" :key="e.section" role="menuitem" :title="t('memmap.listHint')" @click="showStructure(e.section)"><span>{{ e.label }}</span><span class="mono muted">{{ e.where }}</span></button>
        </div>
      </div>
    </div>
    <div class="strip">
      <div v-for="b in blocks" :key="b.name" class="block" :class="b.kind" :style="{ width: b.width + '%' }" @pointerenter="show(blockTip(b), $event)" @pointermove="move">
        <button
          v-for="seg in layout(b)"
          :key="seg.label + seg.from"
          class="seg"
          :class="[seg.cls, { faded: seg.faded }]"
          :style="{ left: seg.left + '%', width: seg.width + '%' }"
          :aria-label="`${seg.label} ${hex0x(seg.from)}`"
          @pointerenter.stop="show(segmentTip(seg), $event)"
          @pointerleave="show(blockTip(b), $event)"
          @click="showInMemory(seg.from)"
        />
      </div>
    </div>
    <div class="names">
      <div v-for="b in blocks" :key="b.name" class="block-name" :style="{ width: b.width + '%' }">
        <span>{{ b.name }}</span>
        <small class="muted mono">{{ hex0x(b.base) }}</small>
      </div>
    </div>

    <p class="title second">{{ t("memmap.realOrSimulated") }}</p>
    <div class="strip">
      <div v-for="b in blocks" :key="b.name" class="block" :class="b.kind" :style="{ width: b.width + '%' }" @pointerenter="show(blockTip(b), $event)" @pointermove="move">
        <button
          v-for="seg in layout(b, true)"
          :key="seg.label + seg.from"
          class="seg"
          :class="seg.kind"
          :style="{ left: seg.left + '%', width: seg.width + '%' }"
          :aria-label="`${seg.label} ${hex0x(seg.from)}`"
          @pointerenter.stop="show(segmentTip(seg), $event)"
          @pointerleave="show(blockTip(b), $event)"
          @click="showInMemory(seg.from)"
        />
        <span v-if="b.kind === 'reserved' || b.name.includes('itcm') || b.name.includes('internal')" class="seg simulated whole" />
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

    <template v-if="markers.length">
      <p class="title">{{ t("memmap.structures") }}</p>
      <ul class="structs">
        <li v-for="m in markers" :key="m.section">
          <button :title="t('memmap.markerHint', { name: m.label, address: hex0x(m.address) })" @click="showStructure(m.section)">{{ m.label }} <span class="mono muted">{{ hex0x(m.address) }}</span></button>
        </li>
      </ul>
    </template>
    <p class="muted note">{{ t("memmap.note") }}</p>

    <div v-if="tip" class="tip" :class="tip.kind" :style="{ left: tip.x + 'px', top: tip.y + 'px' }" role="tooltip">
      <strong>{{ tip.title }}</strong>
      <span v-if="tip.kind" class="tag">{{ t(tip.kind === "real" ? "memmap.realTag" : "memmap.simulatedTag") }}</span>
      <span v-for="(line, i) in tip.lines" :key="i" class="line" :class="{ mono: i === 0 }">{{ line }}</span>
    </div>
  </div>
</template>

<style scoped>
.map { position: relative; display: grid; grid-template-columns: minmax(0, 1fr); gap: 4px; }
.marks { position: relative; height: 66px; margin-top: 4px; }
.lab { padding: 0 6px; font-size: 11px; height: 18px; border-radius: 4px; position: relative; z-index: 2; }
.mark { position: absolute; top: 0; bottom: 0; width: 0; }
.mark .lab { position: absolute; top: 0; transform: translateX(-50%); }
.mark.start .lab { transform: none; left: -6px; }
.mark.end .lab { transform: none; left: auto; right: -6px; }
.mark .stem { position: absolute; left: 0; top: 18px; bottom: 0; width: 1px; background: var(--muted); }
/* Une plage : libellé sur la ligne du bas, trait vertical jusqu'au milieu de l'accolade. */
.range { position: absolute; bottom: 0; height: 44px; min-width: 6px; }
.range .lab { position: absolute; top: 0; left: 50%; transform: translateX(-50%); }
.range.start .lab { left: 0; transform: none; }
.range.end .lab { left: auto; right: 0; transform: none; }
.range .rstem { position: absolute; left: 50%; top: 18px; bottom: 9px; width: 1px; background: var(--muted); }
.brace { position: absolute; left: 0; right: 0; bottom: 0; height: 9px; border: 1.5px solid var(--muted); border-bottom: none; border-radius: 4px 4px 0 0; }
.range:hover .brace, .mark:hover .stem { border-color: var(--accent); background-color: var(--accent); }
.range:hover .brace { background-color: transparent; }
.range:hover .rstem { background: var(--accent); }
/* La liste qui se déroule sous le libellé : une barre verte courte en haut, comme une fiche. */
.pop { display: none; position: absolute; z-index: 6; top: 18px; min-width: 210px; padding: 6px 0 4px; background: var(--surface); color: var(--text); border: 1px solid var(--text); border-radius: 3px; box-shadow: 3px 3px 0 rgb(0 0 0 / 18%); }
.pop::before { content: ""; position: absolute; left: -1px; top: -1px; width: 40px; height: 3px; background: var(--accent); }
.mark .pop { left: 0; transform: translateX(-50%); }
.mark.start .pop { transform: none; left: -6px; }
.mark.end .pop { transform: none; left: auto; right: -6px; }
.range .pop { left: 50%; transform: translateX(-50%); }
.range.start .pop { left: 0; transform: none; }
.range.end .pop { left: auto; right: 0; transform: none; }
.mark:hover .pop, .mark:focus-within .pop, .range:hover .pop, .range:focus-within .pop { display: block; }
.pop button { display: grid; width: 100%; gap: 1px; justify-content: start; text-align: left; border: none; border-radius: 0; background: transparent; padding: 4px 10px; font-size: 12px; }
.pop button:hover { background-color: var(--accent-soft); }
.pop .muted { font-size: 11px; }
.title { margin: 6px 0 2px; font-size: 12px; color: var(--muted); }
.title.second { margin-top: 20px; }
/* Un seul rectangle arrondi par carte ; les zones de la mémoire y sont côte à côte, séparées d'un trait. */
.strip { display: flex; height: 32px; border: 1px solid var(--line); border-radius: 5px; overflow: hidden; background: var(--surface-2); }
.block { position: relative; flex: none; background: var(--surface-2); }
.block + .block { border-left: 2px solid var(--surface); }
.block.reserved { background: repeating-linear-gradient(45deg, var(--surface-2), var(--surface-2) 4px, var(--line) 4px, var(--line) 8px); }
.seg { position: absolute; top: 0; bottom: 0; min-width: 2px; border: none; border-radius: 0; padding: 0; opacity: 0.95; cursor: pointer; }
.seg:hover { opacity: 1; filter: brightness(1.12); }
.seg.faded { opacity: 0.28; }
.seg.app { background: #3f7a1f; } .seg.data { background: #6e9f2a; } .seg.heap { background: #b5dd78; } .seg.stack { background: #e3a008; }
.seg.files { background: #2b6cb0; } .seg.system { background: #7b8794; } .seg.appsflash { background: #3f7a1f; }
.seg.real { background: var(--accent); } .seg.simulated { background: repeating-linear-gradient(45deg, #7b8794, #7b8794 3px, #a8b2bd 3px, #a8b2bd 6px); }
.seg.whole { left: 0; width: 100%; pointer-events: none; }
.names { display: flex; }
.names .block-name { display: grid; font-size: 11px; line-height: 1.2; overflow: hidden; flex: none; padding: 0 2px; }
.names .block-name span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.legend { list-style: none; margin: 14px 0 0; padding: 0; display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 12px; }
.sw { display: inline-block; width: 12px; height: 12px; border-radius: 3px; vertical-align: -2px; }
.sw.app { background: #3f7a1f; } .sw.data { background: #6e9f2a; } .sw.heap { background: #b5dd78; } .sw.stack { background: #e3a008; }
.sw.files { background: #2b6cb0; } .sw.system { background: #7b8794; } .sw.real { background: var(--accent); }
.sw.simulated { background: repeating-linear-gradient(45deg, #7b8794, #7b8794 3px, #a8b2bd 3px, #a8b2bd 6px); }
.structs { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px; }
.structs button { font-size: 12px; }
.note { margin: 14px 0 0; font-size: 12px; }
/* L'infobulle : une boîte carrée, avec une petite pointe vers le curseur, qui suit la souris. */
.tip {
  position: absolute; z-index: 5; width: 262px; display: grid; gap: 3px; padding: 7px 9px; pointer-events: none; font-size: 12px; line-height: 1.35;
  background: var(--surface); color: var(--text); border: 1px solid var(--text); border-radius: 3px; box-shadow: 3px 3px 0 rgb(0 0 0 / 18%);
}
.tip::before { content: ""; position: absolute; left: 10px; top: -7px; width: 11px; height: 11px; background: var(--surface); border-left: 1px solid var(--text); border-top: 1px solid var(--text); transform: rotate(45deg); }
.tip strong { font-size: 13px; }
.tip .tag { justify-self: start; font-size: 10.5px; padding: 0 6px; border-radius: 3px; background: var(--accent-soft); color: var(--accent-strong); }
.tip.simulated .tag { background: color-mix(in srgb, var(--muted) 20%, transparent); color: var(--muted); }
.tip .line:last-child { color: var(--muted); }
</style>
