<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { confirmDialog, promptDialog } from "../dialogs.js";
import { ui } from "../ui.js";
import Icon from "./Icon.vue";
import { hex, hex0x, parseAddress } from "../util.js";

// Sans autre indication, l'éditeur lit et écrit la mémoire de l'émulateur (modifiable seulement en pause). Avec
// `read(adresse, longueur)` et `write(adresse, octets)`, il édite autre chose : les octets d'un fichier, par exemple.
// `version` change quand ces octets ont changé ailleurs.
export interface HexRegion {
  name: string;
  base: number;
  size: number;
}

const props = withDefaults(
  defineProps<{
    regions: HexRegion[];
    read?: ((address: number, length: number) => number[] | null) | null;
    write?: ((address: number, data: number[]) => boolean) | null;
    editable?: boolean | null;
    version?: number;
    height?: number;
  }>(),
  { read: null, write: null, editable: null, version: 0, height: 340 },
);
const { t } = useI18n();
const emulator = useEmu();
const { state, quiet, call } = emulator;

const ROW = 20; // hauteur d'une ligne, en pixels
const COLUMNS = 16;
const paused = computed(() => props.editable ?? !state.running);

function fetchBytes(address: number, length: number): { ok: true; bytes: number[] } | { ok: false; error: string } {
  if (props.read) {
    const bytes = props.read(address, length);
    return bytes ? { ok: true, bytes } : { ok: false, error: "" };
  }
  const reply = quiet({ cmd: "memory_read", address, length });
  return reply.ok ? { ok: true, bytes: (reply.hex.match(/../g) ?? []).map((b: string) => parseInt(b, 16)) } : { ok: false, error: String(reply.error ?? "") };
}

const regionIndex = ref(0);
const region = computed((): HexRegion | undefined => props.regions[regionIndex.value]);
const rowCount = computed(() => (region.value ? Math.ceil(region.value.size / COLUMNS) : 0));
const scroller = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewRows = ref(24);
const group = ref(1);
const bigEndian = ref(false);
const gotoText = ref("");
const findText = ref("");
const findMessage = ref("");
const cursor = ref<number | null>(null); // adresse du curseur
const anchor = ref<number | null>(null); // début de la sélection
const pending = ref(""); // premier chiffre d'un octet en cours de saisie
const error = ref("");

const firstRow = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW)));
const baseAddress = computed(() => (region.value?.base ?? 0) + firstRow.value * COLUMNS);

// ---- lecture --------------------------------------------------------------------------------------

const bytes = ref<number[]>([]);
const previous = ref<number[] & { base?: number }>([]);
const changed = reactive(new Set<number>());

function readWindow() {
  const r = region.value;
  if (!r) return;
  const rows = Math.min(viewRows.value + 1, rowCount.value - firstRow.value);
  const length = Math.max(0, Math.min(rows * COLUMNS, r.base + r.size - baseAddress.value));
  if (length === 0) return void (bytes.value = []);
  const reply = fetchBytes(baseAddress.value, length);
  if (!reply.ok) {
    error.value = reply.error;
    bytes.value = [];
    return;
  }
  error.value = "";
  const next = reply.bytes;
  // Les octets qui ont changé depuis la lecture précédente de la même fenêtre sont mis en évidence.
  changed.clear();
  if (previous.value.length === next.length && previous.value.base === baseAddress.value) {
    next.forEach((b, i) => b !== previous.value[i] && changed.add(baseAddress.value + i));
  }
  previous.value = Object.assign(next, { base: baseAddress.value });
  bytes.value = next;
}

let timer = 0;
onMounted(() => {
  measure();
  readWindow();
  timer = setInterval(readWindow, 500);
  addEventListener("resize", measure);
});
onBeforeUnmount(() => {
  clearInterval(timer);
  removeEventListener("resize", measure);
});
watch([baseAddress, regionIndex, () => state.revision, () => props.version], readWindow);
watch(() => props.regions.length, () => {
  if (regionIndex.value >= props.regions.length) regionIndex.value = 0;
  readWindow();
});

function measure() {
  if (scroller.value) viewRows.value = Math.ceil(scroller.value.clientHeight / ROW) + 1;
}
function onScroll() {
  if (scroller.value) scrollTop.value = scroller.value.scrollTop;
}

// ---- se déplacer ----------------------------------------------------------------------------------

function regionOf(address: number) {
  return props.regions.findIndex((r) => address >= r.base && address < r.base + r.size);
}

async function goTo(address: number, select = true) {
  const index = regionOf(address);
  if (index < 0) {
    error.value = t("hex.unmapped", { address: hex0x(address) });
    return;
  }
  error.value = "";
  regionIndex.value = index;
  await nextTick();
  const row = Math.floor((address - (props.regions[index]?.base ?? 0)) / COLUMNS);
  if (scroller.value) scroller.value.scrollTop = Math.max(0, row * ROW - ROW * 3);
  onScroll();
  if (select) {
    cursor.value = anchor.value = address;
    pending.value = "";
  }
  readWindow();
}

function submitGoto() {
  const address = parseAddress(gotoText.value);
  if (address === undefined) error.value = t("hex.badAddress");
  else goTo(address);
}

watch(
  () => ui.hexGoto,
  (request) => request && !props.read && goTo(request.address),
  { immediate: true },
);
watch(regionIndex, async () => {
  if (cursor.value !== null && regionOf(cursor.value) === regionIndex.value) return;
  await nextTick();
  if (scroller.value) scroller.value.scrollTop = 0;
  onScroll();
});

// ---- sélection, saisie ----------------------------------------------------------------------------

const selection = computed(() => {
  if (cursor.value === null) return null;
  const a = anchor.value ?? cursor.value;
  return { start: Math.min(a, cursor.value), end: Math.max(a, cursor.value) };
});
const inSelection = (address: number) => selection.value && address >= selection.value.start && address <= selection.value.end;

let dragging = false;
function pointerDown(address: number, event: PointerEvent) {
  (event.currentTarget as Element).closest<HTMLElement>(".grid")?.focus({ preventScroll: true });
  cursor.value = address;
  if (!event.shiftKey) anchor.value = address;
  pending.value = "";
  dragging = true;
}
function pointerEnter(address: number) {
  if (dragging) cursor.value = address;
}
const stopDrag = () => (dragging = false);
onMounted(() => addEventListener("pointerup", stopDrag));
onBeforeUnmount(() => removeEventListener("pointerup", stopDrag));

function move(delta: number, extend: boolean) {
  const r = region.value;
  if (!r || cursor.value === null) return;
  const next = Math.max(r.base, Math.min(r.base + r.size - 1, cursor.value + delta));
  cursor.value = next;
  if (!extend) anchor.value = next;
  pending.value = "";
  ensureVisible(next);
}
function ensureVisible(address: number) {
  const row = Math.floor((address - (region.value?.base ?? 0)) / COLUMNS);
  const top = row * ROW;
  const el = scroller.value;
  if (!el) return;
  if (top < el.scrollTop) el.scrollTop = top;
  else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight;
  onScroll();
}

function write(address: number, data: number[]) {
  if (!paused.value) return false;
  if (props.write) {
    if (!props.write(address, data)) return false;
  } else {
    const reply = call({ cmd: "memory_write", address, hex: data.map((b: number) => hex(b, 2)).join("") });
    if (!reply.ok) return false;
    emulator.bump();
  }
  readWindow();
  return true;
}

function keydown(event: KeyboardEvent) {
  const step = ({ ArrowLeft: -1, ArrowRight: 1, ArrowUp: -COLUMNS, ArrowDown: COLUMNS, PageUp: -COLUMNS * (viewRows.value - 2), PageDown: COLUMNS * (viewRows.value - 2) } as Record<string, number>)[event.key];
  if (step !== undefined) {
    move(step, event.shiftKey);
    return event.preventDefault();
  }
  if (event.key === "Home" && cursor.value !== null) {
    move(-(cursor.value - (region.value?.base ?? 0)) % COLUMNS, event.shiftKey);
    return event.preventDefault();
  }
  if (event.key === "End" && cursor.value !== null) {
    move(COLUMNS - 1 - ((cursor.value - (region.value?.base ?? 0)) % COLUMNS), event.shiftKey);
    return event.preventDefault();
  }
  if (event.key === "Escape") {
    pending.value = "";
    anchor.value = cursor.value;
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
    copy(event.shiftKey ? "ascii" : "hex");
    return event.preventDefault();
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a" && region.value) {
    anchor.value = region.value.base;
    cursor.value = region.value.base + region.value.size - 1;
    return event.preventDefault();
  }
  if (/^[0-9a-f]$/i.test(event.key) && !event.ctrlKey && !event.metaKey && !event.altKey && cursor.value !== null) {
    event.preventDefault();
    if (!paused.value) return void (error.value = t("hex.pauseToEdit"));
    if (!pending.value) {
      pending.value = event.key;
    } else {
      const value = parseInt(pending.value + event.key, 16);
      pending.value = "";
      if (write(cursor.value, [value])) move(1, false);
    }
  }
}

async function paste(event: ClipboardEvent) {
  if (cursor.value === null || !paused.value) return;
  const text = (event.clipboardData?.getData("text") ?? "").replace(/0x/gi, "").replace(/[^0-9a-f]/gi, "");
  if (text.length < 2) return;
  event.preventDefault();
  const data = (text.match(/../g) ?? []).map((b) => parseInt(b, 16));
  if (data.length > 1 && !(await confirmDialog(t("hex.pasteConfirm", { count: data.length, address: hex0x(cursor.value) })))) return;
  write(cursor.value, data);
}

// ---- actions --------------------------------------------------------------------------------------

function selectedBytes() {
  const s = selection.value;
  if (!s || s.end - s.start > 1 << 20) return null;
  const reply = fetchBytes(s.start, s.end - s.start + 1);
  return reply.ok ? reply.bytes : null;
}

async function copy(kind: "hex" | "ascii") {
  const data = selectedBytes();
  if (!data) return;
  const text = kind === "ascii" ? data.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : ".")).join("") : data.map((b) => hex(b, 2)).join(" ");
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    error.value = t("hex.copyFailed");
  }
}

async function fill() {
  const s = selection.value;
  if (!s) return;
  const answer = await promptDialog(t("hex.fillValue", { count: s.end - s.start + 1 }), "00", { title: t("hex.fill"), accept: t("hex.fill") });
  if (answer === null) return;
  const value = parseInt(answer.replace(/^0x/i, ""), 16);
  if (!(value >= 0 && value <= 255)) return void (error.value = t("hex.badByte"));
  const length = s.end - s.start + 1;
  const CHUNK = 1 << 16;
  for (let at = 0; at < length; at += CHUNK) {
    if (!write(s.start + at, Array(Math.min(CHUNK, length - at)).fill(value))) break;
  }
}

// Recherche d'une suite d'octets (« de ad be ef » ou « "texte" ») à partir du curseur, dans la zone courante.
let searching = false;
function parsePattern(text: string): number[] | null {
  const t0 = text.trim();
  const quoted = t0.match(/^"(.*)"$/s);
  if (quoted) return Array.from(new TextEncoder().encode(quoted[1]));
  const digits = t0.replace(/0x/gi, "").replace(/[\s,]/g, "");
  return /^([0-9a-f]{2})+$/i.test(digits) ? (digits.match(/../g) ?? []).map((b) => parseInt(b, 16)) : null;
}

async function find(direction = 1) {
  const pattern = parsePattern(findText.value);
  const r = region.value;
  if (!pattern?.length) return void (findMessage.value = t("hex.badPattern"));
  if (!r || searching) return;
  searching = true;
  findMessage.value = t("hex.searching");
  const CHUNK = 1 << 16;
  const overlap = pattern.length - 1;
  const from = (cursor.value ?? r.base - (direction > 0 ? 1 : -1)) + direction;
  let at = Math.max(r.base, Math.min(from, r.base + r.size - 1));
  let found = null;
  for (let scanned = 0; scanned < r.size && found === null; scanned += CHUNK) {
    const start = direction > 0 ? at : Math.max(r.base, at - CHUNK + 1);
    const length = Math.min(CHUNK + overlap, r.base + r.size - start);
    if (length <= 0) break;
    const reply = fetchBytes(start, length);
    if (!reply.ok) break;
    const data = reply.bytes;
    const order = direction > 0 ? [...data.keys()] : [...data.keys()].reverse();
    for (const i of order) {
      if (direction > 0 ? start + i < at : start + i > at) continue;
      if (pattern.every((b, k) => data[i + k] === b)) {
        found = start + i;
        break;
      }
    }
    at = direction > 0 ? start + CHUNK : start - 1;
    if (at < r.base || at >= r.base + r.size) break;
    await new Promise((resolve) => setTimeout(resolve));
  }
  searching = false;
  if (found === null) return void (findMessage.value = t("hex.notFound"));
  findMessage.value = t("hex.foundAt", { address: hex0x(found) });
  await goTo(found);
  anchor.value = found;
  cursor.value = found + pattern.length - 1;
}

// ---- affichage ------------------------------------------------------------------------------------

const rows = computed(() => {
  const out = [];
  const data = bytes.value;
  for (let i = 0; i < data.length; i += COLUMNS) {
    const address = baseAddress.value + i;
    const cells = [];
    for (let j = 0; j < COLUMNS && i + j < data.length; j++) cells.push({ address: address + j, value: data[i + j] ?? 0 });
    out.push({ address, cells });
  }
  return out;
});
const printable = (b: number) => (b >= 32 && b < 127 ? String.fromCharCode(b) : "·");
const gap = (j: number) => (group.value > 1 && j > 0 && j % group.value === 0 ? "gap" : "");

// Valeurs lues à partir du curseur (inspecteur).
const inspector = computed(() => {
  if (cursor.value === null) return null;
  const reply = fetchBytes(cursor.value, Math.min(8, (region.value?.base ?? 0) + (region.value?.size ?? 0) - cursor.value));
  if (!reply.ok) return null;
  const data = new Uint8Array(reply.bytes);
  // `state.revision` et `bytes` forcent le recalcul quand la mémoire change.
  void state.revision;
  void bytes.value;
  const view = new DataView(data.buffer);
  const le = !bigEndian.value;
  const has = (n: number) => data.length >= n;
  return {
    u8: data[0], i8: view.getInt8(0),
    u16: has(2) ? view.getUint16(0, le) : null, i16: has(2) ? view.getInt16(0, le) : null,
    u32: has(4) ? view.getUint32(0, le) : null, i32: has(4) ? view.getInt32(0, le) : null,
    f32: has(4) ? view.getFloat32(0, le) : null, f64: has(8) ? view.getFloat64(0, le) : null,
    bin: (data[0] ?? 0).toString(2).padStart(8, "0"),
    ascii: printable(data[0] ?? 0),
  };
});
const num = (v: unknown) => (v === null || v === undefined ? "—" : String(v));
</script>

<template>
  <section class="hex" :aria-label="t('hex.label')">
    <div class="toolbar" role="toolbar" :aria-label="t('hex.toolbar')">
      <label v-if="regions.length > 1" class="field">{{ t("hex.region") }}
        <select v-model="regionIndex">
          <option v-for="(r, i) in regions" :key="r.name" :value="i">{{ r.name }} · {{ hex0x(r.base) }}</option>
        </select>
      </label>
      <label class="field">{{ t("hex.goto") }}
        <input v-model="gotoText" class="mono" :placeholder="read ? '0x0' : '0x20000000'" data-keep-keys @keydown.enter="submitGoto" />
      </label>
      <label class="field">{{ t("hex.find") }}
        <span class="find">
          <input v-model="findText" class="mono" placeholder="de ad be ef  ·  &quot;text&quot;" data-keep-keys @keydown.enter="find(1)" />
          <button class="icon" :title="t('hex.findPrev')" :aria-label="t('hex.findPrev')" @click="find(-1)">▲</button>
          <button class="icon" :title="t('hex.findNext')" :aria-label="t('hex.findNext')" @click="find(1)">▼</button>
        </span>
      </label>
      <label class="field">{{ t("hex.group") }}
        <select v-model.number="group"><option :value="1">1</option><option :value="2">2</option><option :value="4">4</option><option :value="8">8</option></select>
      </label>
    </div>
    <div class="actions">
      <button :disabled="!selection" @click="copy('hex')">{{ t("hex.copyHex") }}</button>
      <button :disabled="!selection" @click="copy('ascii')">{{ t("hex.copyAscii") }}</button>
      <button :disabled="!selection || !paused" :title="paused ? '' : t('common.pauseRequired')" @click="fill">{{ t("hex.fill") }}</button>
      <span v-if="selection" class="muted mono">{{ hex0x(selection.start) }}{{ selection.end > selection.start ? ` → ${hex0x(selection.end)} (${selection.end - selection.start + 1})` : "" }}</span>
      <span class="spacer" />
      <span v-if="findMessage" class="muted">{{ findMessage }}</span>
    </div>
    <p v-if="!paused" class="banner" role="status"><Icon name="pause" :size="12" /> {{ t("hex.pauseToEdit") }}</p>
    <p v-if="error" class="error">{{ error }}</p>

    <div class="viewer">
      <div class="head mono" aria-hidden="true">
        <span class="addr">{{ t("hex.address") }}</span>
        <span class="cells"><span v-for="j in COLUMNS" :key="j" :class="gap(j - 1)">{{ hex(j - 1, 2) }}</span></span>
        <span class="ascii">ASCII</span>
      </div>
      <div ref="scroller" class="scroller" :style="{ height: height + 'px' }" @scroll.passive="onScroll">
        <div class="spacer-v" :style="{ height: rowCount * ROW + 'px' }">
          <div
            class="grid mono"
            tabindex="0"
            role="grid"
            :aria-label="t('hex.grid')"
            :aria-rowcount="rowCount"
            data-keep-keys
            :style="{ transform: `translateY(${firstRow * ROW}px)` }"
            @keydown="keydown"
            @paste="paste"
          >
            <div v-for="row in rows" :key="row.address" class="row" role="row" :style="{ height: ROW + 'px' }">
              <span class="addr">{{ hex(row.address) }}</span>
              <span class="cells">
                <span
                  v-for="(cell, j) in row.cells"
                  :key="cell.address"
                  role="gridcell"
                  :class="['cell', gap(j), { sel: inSelection(cell.address), cur: cell.address === cursor, chg: changed.has(cell.address), zero: cell.value === 0 }]"
                  @pointerdown.prevent="pointerDown(cell.address, $event)"
                  @pointerenter="pointerEnter(cell.address)"
                >{{ cell.address === cursor && pending ? pending + "_" : hex(cell.value ?? 0, 2) }}</span>
              </span>
              <span class="ascii">
                <span
                  v-for="cell in row.cells"
                  :key="cell.address"
                  :class="['ch', { sel: inSelection(cell.address), cur: cell.address === cursor, chg: changed.has(cell.address) }]"
                  @pointerdown.prevent="pointerDown(cell.address, $event)"
                  @pointerenter="pointerEnter(cell.address)"
                >{{ printable(cell.value) }}</span>
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="inspector" class="inspector mono" :aria-label="t('hex.inspector')">
      <span>u8 {{ inspector.u8 }}</span><span>i8 {{ inspector.i8 }}</span>
      <span>u16 {{ num(inspector.u16) }}</span><span>i16 {{ num(inspector.i16) }}</span>
      <span>u32 {{ num(inspector.u32) }}</span><span>i32 {{ num(inspector.i32) }}</span>
      <span>f32 {{ num(inspector.f32) }}</span><span>f64 {{ num(inspector.f64) }}</span>
      <span>bin {{ inspector.bin }}</span><span>chr {{ inspector.ascii }}</span>
      <label class="endian"><input v-model="bigEndian" type="checkbox" data-keep-keys /> {{ t("hex.bigEndian") }}</label>
    </div>
    <p class="muted hint">{{ t("hex.help") }}</p>
  </section>
</template>

<style scoped>
.hex { display: grid; gap: 14px; }
.toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: end; }
.find { display: flex; gap: 4px; }
.find input { width: 14em; }
.actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.spacer { flex: 1; }
.banner { margin: 0; padding: 4px 8px; background: color-mix(in srgb, var(--warn) 16%, transparent); border-radius: 6px; font-size: 12px; }
.viewer { border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--surface-2); }
.head { display: flex; gap: 14px; padding: 4px 10px; color: var(--muted); border-bottom: 1px solid var(--line); background: var(--surface); }
.scroller { overflow: auto; position: relative; }
.spacer-v { position: relative; }
.grid { position: absolute; left: 0; right: 0; top: 0; outline: none; padding: 0 10px; }
.grid:focus-visible { box-shadow: inset 0 0 0 2px var(--focus); }
.row { display: flex; gap: 14px; align-items: center; white-space: pre; user-select: none; }
.addr { color: var(--muted); width: 8ch; flex: none; }
.cells { display: flex; flex: none; width: calc(16 * 2.5ch + 6ch); }
.head .cells span { width: 2.5ch; text-align: center; }
.cell { width: 2.5ch; text-align: center; cursor: default; border-radius: 3px; }
.gap { margin-left: 1ch; }
.cell.zero { color: var(--muted); }
.cell.sel, .ch.sel { background: var(--accent-soft); }
.cell.cur, .ch.cur { background: var(--accent); color: var(--accent-text); }
.cell.chg, .ch.chg { color: var(--warn); font-weight: 700; }
.ascii { display: inline-flex; flex: none; width: 16ch; }
.ch { width: 1ch; }
.inspector { display: flex; flex-wrap: wrap; gap: 4px 14px; padding: 6px 8px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); }
.endian { font-family: system-ui, sans-serif; font-size: 12px; margin-left: auto; }
.hint { margin: 0; font-size: 12px; }
</style>
