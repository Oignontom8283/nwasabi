<script setup lang="ts">
// Le tableau du code désassemblé : une ligne par instruction, sur une grille de colonnes fixes (rien ne défile de côté).
//
//   marge des flèches | point d'arrêt | genre de saut | adresse | octets | instruction | actions
//
// Il se charge de tout ce qui est propre au tableau : la sélection de lignes (clic, plage, rectangle, flèches), la flèche qui va d'un
// saut à sa cible au survol, l'instruction qu'on vient de rejoindre, et les raccourcis. Ce qu'il faut faire d'une ligne (poser un point
// d'arrêt, suivre un saut, ouvrir un menu…) revient au parent par des événements.
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRowSelection } from "../../composables/useRowSelection.js";
import { jumpKind, tooltip } from "../../thumbDocs.js";
import type { JumpKind } from "../../thumbDocs.js";
import { hex } from "../../util.js";
import Icon from "../Icon.vue";
import type { AsmLine } from "./asm.js";

const props = defineProps<{
  lines: AsmLine[];
  /** Adresse de l'instruction en cours. */
  pc: number | null;
  breaks: number[];
  /** Plage d'octets à repérer. */
  zone: { address: number; length: number } | null;
  /** L'application est en pause : on peut exécuter jusqu'à une adresse. */
  paused: boolean;
  /** Instruction à rejoindre (la sélectionner, la montrer et la signaler) : le parent la remet à `null` ensuite. */
  focusAddress: number | null;
}>();
const emit = defineEmits<{
  toggleBreak: [address: number];
  runTo: [address: number];
  follow: [address: number];
  memory: [address: number];
  menu: [at: { x: number; y: number }, line: AsmLine, chosen: AsmLine[]];
  copy: [chosen: AsmLine[]];
  focused: [];
}>();
const { t } = useI18n();

const rows = ref<HTMLElement | null>(null);
const sel = useRowSelection<number>({ container: rows, keys: () => props.lines.map((l) => l.address), attribute: "data-addr", parse: Number });
const chosen = computed(() => props.lines.filter((l) => sel.selected.value.has(l.address)));
const inZone = (a: number) => props.zone !== null && a >= props.zone.address && a < props.zone.address + props.zone.length;
const rowOf = (address: number) => rows.value?.querySelector<HTMLElement>(`[data-addr="${address}"]`) ?? null;

const kindOf = (l: AsmLine): JumpKind | null => jumpKind(l.mnemonic, l.operands, l.target !== undefined);
const rowTitle = (l: AsmLine) => {
  const doc = tooltip(l.mnemonic, l.operands, l.target !== undefined);
  return `${l.mnemonic} ${l.operands}${doc ? "\n" + doc : ""}`.trim();
};

// ---- l'instruction qu'on vient de rejoindre -----------------------------------------------------------------------------------------
const flash = ref<number | null>(null);
let flashTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => [props.lines, props.focusAddress] as const,
  async () => {
    const address = props.focusAddress;
    if (address === null) return;
    await nextTick();
    const row = rowOf(address);
    if (!row) return;
    emit("focused");
    sel.only(address);
    row.scrollIntoView({ block: "nearest" });
    flash.value = address;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash.value = null), 1600);
  },
);

// ---- la flèche d'un saut à sa cible -------------------------------------------------------------------------------------------------
// Elle se dessine dans la marge de gauche : un trait horizontal depuis la ligne du saut, un trait vertical, puis un trait horizontal avec
// la pointe sur la ligne visée. Une cible hors de la liste reçoit une pointe vers le haut ou le bas, au bord, avec son nom.
interface Arrow {
  kind: JumpKind;
  from: number;
  to: number | null;
  edge: "up" | "down" | null;
  label: string;
  /** Largeur de la marge au moment où la flèche est dessinée. */
  lane: number;
  height: number;
}
const hovered = ref<number | null>(null);
const arrow = ref<Arrow | null>(null);

const middleOf = (address: number): number | null => {
  const row = rowOf(address);
  return row ? row.offsetTop + row.offsetHeight / 2 : null;
};

function drawArrow(line: AsmLine | undefined) {
  const kind = line ? kindOf(line) : null;
  const area = rows.value;
  if (!line || !kind || line.target === undefined || !area) return void (arrow.value = null);
  const from = middleOf(line.address);
  if (from === null) return void (arrow.value = null);
  const to = middleOf(line.target);
  const lane = area.querySelector<HTMLElement>("[data-lane]")?.offsetWidth ?? 40;
  const first = props.lines[0]?.address ?? 0;
  arrow.value = {
    kind,
    from,
    to,
    edge: to === null ? (line.target < first ? "up" : "down") : null,
    label: line.target_symbol ?? `0x${hex(line.target)}`,
    lane,
    height: area.scrollHeight,
  };
}
// La flèche suit la souris ; sans souris sur une ligne, elle suit la ligne active du clavier.
const arrowLine = computed(() => props.lines.find((l) => l.address === (hovered.value ?? sel.caret.value)));
watch([arrowLine, () => props.lines], () => drawArrow(arrowLine.value), { flush: "post" });

const ARROW_HEAD = 7;
const geometry = computed(() => {
  const a = arrow.value;
  if (!a) return null;
  const right = a.lane - 3; // où la flèche touche les lignes
  const left = 9; // le trait vertical
  const start = { x: right, y: a.from };
  if (a.to === null) {
    const y = a.edge === "up" ? 4 : a.height - 4;
    const tip = a.edge === "up" ? y : y;
    const base = a.edge === "up" ? y + ARROW_HEAD : y - ARROW_HEAD;
    return { start, path: `M ${right} ${a.from} H ${left} V ${base}`, head: `${left},${tip} ${left - 4},${base} ${left + 4},${base}` };
  }
  if (a.to === a.from) return { start, path: "", head: `${right},${a.to} ${right - ARROW_HEAD},${a.to - 4} ${right - ARROW_HEAD},${a.to + 4}` };
  return { start, path: `M ${right} ${a.from} H ${left} V ${a.to} H ${right - ARROW_HEAD}`, head: `${right},${a.to} ${right - ARROW_HEAD},${a.to - 4} ${right - ARROW_HEAD},${a.to + 4}` };
});

// ---- sélection, clavier ---------------------------------------------------------------------------------------------------------------
function rowClick(event: MouseEvent, line: AsmLine) {
  if (sel.wasDrag() || (event.target as Element).closest("button, a, input") || getSelection()?.toString()) return;
  sel.click(line.address, event);
}

function openMenu(event: { clientX: number; clientY: number }, line: AsmLine) {
  if (!sel.selected.value.has(line.address)) sel.only(line.address);
  emit("menu", { x: event.clientX, y: event.clientY }, line, chosen.value);
}

/** Flèches (Maj : étend), Ctrl + A, Ctrl + C, Échap, et sur la ligne active : F9 point d'arrêt, Entrée suit le saut, menu. */
function onKey(event: KeyboardEvent) {
  const ctrl = event.ctrlKey || event.metaKey;
  const active = props.lines.find((l) => l.address === sel.caret.value);
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    const next = sel.move(event.key === "ArrowDown" ? 1 : -1, event.shiftKey);
    if (next !== null) rowOf(next)?.scrollIntoView({ block: "nearest" });
  } else if (ctrl && event.key.toLowerCase() === "a") sel.selectAll();
  else if (ctrl && event.key.toLowerCase() === "c") emit("copy", chosen.value);
  else if (event.key === "Escape") sel.clear();
  else if (event.key === "F9" && active) emit("toggleBreak", active.address);
  else if (event.key === "Enter" && active?.target !== undefined) emit("follow", active.target);
  else if ((event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) && active) {
    const box = rowOf(active.address)?.getBoundingClientRect();
    openMenu({ clientX: (box?.left ?? 0) + 80, clientY: box?.bottom ?? 0 }, active);
  } else return;
  event.preventDefault();
}

defineExpose({ clearSelection: sel.clear });
</script>

<template>
  <div class="asm mono" role="table" :aria-label="t('debug.code')">
    <div class="head muted" role="row">
      <span data-lane :title="t('debug.colJumpHint')" />
      <span class="c-center" :title="t('debug.colBreakHint')">
        <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><circle cx="6" cy="6" r="4" /></svg><span class="sr-only">{{ t("debug.colBreak") }}</span>
      </span>
      <span class="c-center" :title="t('debug.colJumpHint')">
        <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M2 3v4h7 M7 4.5L9.5 7 7 9.5" /></svg><span class="sr-only">{{ t("debug.colJump") }}</span>
      </span>
      <span>{{ t("debug.colAddress") }}</span>
      <span class="bytes">{{ t("debug.colBytes") }}</span>
      <span class="span2">{{ t("debug.colInstruction") }}</span>
      <span class="c-center" :title="t('debug.colActionsHint')">{{ t("debug.colActions") }}</span>
    </div>

    <div
      ref="rows"
      class="rows"
      tabindex="0"
      role="rowgroup"
      aria-multiselectable="true"
      :aria-label="t('debug.rowsHint')"
      @pointerdown="sel.pointerDown"
      @pointermove="sel.pointerMove"
      @pointerup="sel.pointerUp"
      @pointercancel="sel.pointerUp"
      @dragstart="sel.dragStart"
      @mouseleave="hovered = null"
      @keydown="onKey"
    >
      <svg v-if="arrow && geometry" class="overlay" :style="{ width: arrow.lane + 'px' }" aria-hidden="true">
        <path v-if="geometry.path" :d="geometry.path" :class="['line', arrow.kind]" />
        <polygon :points="geometry.head" :class="['head-shape', arrow.kind]" />
        <circle :cx="geometry.start.x" :cy="geometry.start.y" r="2.2" :class="['origin', arrow.kind]" />
      </svg>
      <span v-if="arrow?.edge" class="edge" :class="arrow.edge" :style="{ left: arrow.lane + 4 + 'px' }">{{ arrow.edge === "up" ? "↑" : "↓" }} {{ arrow.label }}</span>

      <template v-for="line in lines" :key="line.address">
        <div v-if="line.label" class="label" role="presentation">{{ line.label }}:</div>
        <div
          class="row"
          :class="{ pc: line.address === pc, zone: inZone(line.address), selected: sel.selected.value.has(line.address), flash: flash === line.address }"
          role="row"
          :aria-selected="sel.selected.value.has(line.address)"
          :data-addr="line.address"
          @click="rowClick($event, line)"
          @mouseenter="hovered = line.address"
          @contextmenu.prevent="openMenu($event, line)"
        >
          <span data-lane data-empty aria-hidden="true" />
          <button class="dot" tabindex="-1" :title="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" :aria-label="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" @click="emit('toggleBreak', line.address)">
            <span :class="{ on: breaks.includes(line.address) }" />
          </button>
          <span class="kind" :class="kindOf(line) ?? ''" :title="kindOf(line) ? t(`debug.jump.${kindOf(line) as JumpKind}`) : ''">
            <svg v-if="kindOf(line)" viewBox="0 0 12 12" width="14" height="14" aria-hidden="true">
              <path v-if="kindOf(line) === 'jump' || kindOf(line) === 'branch'" :d="(line.target ?? 0) > line.address ? 'M6 1v8 M3 6.5l3 3 3-3' : 'M6 11V3 M3 5.5l3-3 3 3'" />
              <path v-if="kindOf(line) === 'branch'" d="M1 6l1.5-1.5L4 6 2.5 7.5z" class="solid" />
              <path v-else-if="kindOf(line) === 'call'" d="M3 2l6 4-6 4z" class="solid" />
              <path v-else-if="kindOf(line) === 'return'" d="M10 2v4H3 M5.5 3.5L3 6l2.5 2.5" />
              <path v-else-if="kindOf(line) === 'indirect'" d="M2 6h.01 M6 6h.01 M10 6h.01" />
            </svg>
          </span>
          <button class="addr" tabindex="-1" :title="t('debug.runTo')" :disabled="!paused" @click="emit('runTo', line.address)">{{ hex(line.address) }}</button>
          <span class="bytes muted">{{ line.hex }}</span>
          <span class="mn" :title="rowTitle(line)">{{ line.mnemonic }}</span>
          <span class="ops" :title="rowTitle(line)">
            <span class="text">{{ line.operands }}<span v-if="line.target_symbol" class="muted"> &lt;{{ line.target_symbol }}&gt;</span></span>
            <span class="fill" data-empty />
          </span>
          <span class="actions">
            <button v-if="line.target !== undefined" class="act" tabindex="-1" :title="t('debug.followTarget')" :aria-label="t('debug.followTarget')" @click="emit('follow', line.target)"><Icon name="right" :size="14" /></button>
            <button v-if="line.target !== undefined" class="act" tabindex="-1" :title="t('debug.showInMemory')" :aria-label="t('debug.showInMemory')" @click="emit('memory', line.target)"><Icon name="chip" :size="14" /></button>
          </span>
        </div>
      </template>

      <div v-if="sel.band.value" class="band" :style="{ left: sel.band.value.x + 'px', top: sel.band.value.y + 'px', width: sel.band.value.w + 'px', height: sel.band.value.h + 'px' }" />
      <p v-if="!lines.length" class="muted empty">{{ t("debug.unreadable") }}</p>
    </div>
  </div>
</template>

<style scoped>
/* Le tableau ne défile que de haut en bas : ses colonnes sont fixes, sauf les opérandes qui prennent le reste (et se coupent par « … »). */
.asm {
  --lane: 40px; --cols: var(--lane) 24px 20px 9ch 10ch 9ch minmax(0, 1fr) 64px;
  container: asm / inline-size;
  border: 1px solid var(--line); border-radius: 8px; background: var(--surface); font-size: 12.5px;
  max-height: 440px; overflow-x: hidden; overflow-y: auto;
}
/* Étroit : les octets disparaissent, la marge des flèches se resserre. */
@container asm (max-width: 560px) {
  /* Une requête de conteneur ne touche que l'intérieur du conteneur : la grille se redéfinit sur les lignes elles-mêmes. */
  .head, .row, .label { --lane: 28px; --cols: var(--lane) 24px 20px 9ch 9ch minmax(0, 1fr) 64px; }
  .bytes { display: none; }
}
.head, .row { display: grid; grid-template-columns: var(--cols); column-gap: 6px; align-items: center; white-space: nowrap; }
.head { position: sticky; top: 0; z-index: 3; padding: 4px 0; background: var(--surface-2); border-bottom: 1px solid var(--line); font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; }
.head svg { fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
.c-center { text-align: center; display: grid; place-items: center; }
.span2 { grid-column: span 2; }

.rows { position: relative; outline: none; }
.rows:focus-visible { box-shadow: inset 0 0 0 2px var(--focus); }
.row { height: 26px; position: relative; }
.row:hover { background: var(--accent-soft); }
.row.zone { background: color-mix(in srgb, var(--warn) 14%, transparent); box-shadow: inset 3px 0 0 var(--warn); }
.row.selected { background: color-mix(in srgb, var(--accent) 20%, transparent); }
.row.pc { background: color-mix(in srgb, var(--warn) 22%, transparent); box-shadow: inset 3px 0 0 var(--warn); font-weight: 600; }
.row.selected.pc { background: color-mix(in srgb, var(--warn) 32%, transparent); }
.row.flash { animation: flash 1.6s ease-out; outline: 2px solid var(--accent); outline-offset: -2px; }
@keyframes flash { 0%, 70% { outline-color: var(--accent); } 100% { outline-color: transparent; } }
@media (prefers-reduced-motion: reduce) { .row.flash { animation: none; } }
.label { display: grid; grid-template-columns: var(--cols); padding-top: 4px; color: var(--accent-strong); font-weight: 600; }
.label { grid-template-columns: var(--cols); }
.label::before { content: ""; grid-column: 1 / 4; }
.band { position: absolute; pointer-events: none; z-index: 1; background: color-mix(in srgb, var(--accent) 18%, transparent); border: 1px solid var(--accent); }

.dot { border: none; background: none; padding: 0; width: 24px; height: 24px; display: grid; place-items: center; cursor: pointer; }
.dot span { width: 10px; height: 10px; border-radius: 50%; border: 1px solid transparent; }
.dot:hover span { border-color: var(--danger); }
.dot span.on { background: var(--danger); border-color: var(--danger); }

.kind { display: grid; place-items: center; }
.kind svg { fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
.kind svg .solid { fill: currentColor; stroke: none; }
.kind.jump { color: var(--accent-strong); }
.kind.branch { color: var(--warn); }
.kind.call { color: var(--ok); }
.kind.return, .kind.indirect { color: var(--muted); }

.addr { border: none; background: none; padding: 0; font: inherit; text-align: left; color: var(--accent-strong); cursor: pointer; }
.addr:hover { text-decoration: underline; }
.addr:disabled { color: inherit; cursor: default; text-decoration: none; }
.mn { color: var(--syn-key); cursor: help; overflow: hidden; text-overflow: ellipsis; }
.ops { color: var(--syn-num); cursor: help; display: flex; min-width: 0; }
.ops .text { overflow: hidden; text-overflow: ellipsis; }
.fill { flex: 1; align-self: stretch; }
.actions { display: flex; gap: 4px; justify-content: center; }
.act { width: 28px; height: 22px; padding: 0; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); color: var(--accent-strong); cursor: pointer; }
.act:hover { background: var(--accent-soft); border-color: var(--accent); }

/* La flèche d'un saut, dans la marge de gauche. */
.overlay { position: absolute; left: 0; top: 0; height: 100%; pointer-events: none; z-index: 2; overflow: visible; }
.overlay .line { fill: none; stroke: var(--accent-strong); stroke-width: 1.6; stroke-linejoin: round; }
.overlay .head-shape, .overlay .origin { fill: var(--accent-strong); }
.overlay .branch { stroke: var(--warn); }
.overlay .head-shape.branch, .overlay .origin.branch { fill: var(--warn); stroke: none; }
.overlay .call { stroke: var(--ok); }
.overlay .head-shape.call, .overlay .origin.call { fill: var(--ok); stroke: none; }
.overlay .head-shape, .overlay .origin { stroke: none; }
.edge { position: absolute; z-index: 2; padding: 0 6px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); color: var(--muted); font-size: 11px; pointer-events: none; }
.edge.up { top: 2px; }
.edge.down { bottom: 2px; }
.empty { padding: 10px; margin: 0; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
</style>
