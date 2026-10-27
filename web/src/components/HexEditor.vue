<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { confirmDialog } from "../dialogs.js";
import { notify, ui } from "../ui.js";
import { addWatch } from "../watchStore.js";
import { SCAN_TYPES, SIZE, tile } from "../scan.js";
import type { Base, ScanType } from "../scan.js";
import ExportDialog from "./ExportDialog.vue";
import HexValueDialog from "./HexValueDialog.vue";
import Icon from "./Icon.vue";
import { hex, hex0x, parseAddress, poll } from "../util.js";

// Sans autre indication, l'éditeur lit et écrit la mémoire de l'émulateur (modifiable seulement en pause). Avec
// `read(adresse, longueur)` et `write(adresse, octets)`, il édite autre chose : les octets d'un fichier, par exemple.
// `version` change quand ces octets ont changé ailleurs.
export interface HexRegion {
  name: string;
  base: number;
  size: number;
  /** "ram", "flash" ou "reserved" (réponse `regions` du module) ; absent pour un fichier. */
  kind?: string;
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
/** La mémoire se modifie en direct, application lancée ou non ; un fichier hors ligne peut être refusé (`editable`). */
const canEdit = computed(() => props.editable ?? true);

async function fetchBytes(address: number, length: number): Promise<{ ok: true; bytes: number[] } | { ok: false; error: string }> {
  if (props.read) {
    const bytes = props.read(address, length);
    return bytes ? { ok: true, bytes } : { ok: false, error: "" };
  }
  const reply = await quiet({ cmd: "memory_read", address, length });
  return reply.ok ? { ok: true, bytes: (reply.hex.match(/../g) ?? []).map((b: string) => parseInt(b, 16)) } : { ok: false, error: String(reply.error ?? "") };
}

const regionIndex = ref(0);
const region = computed((): HexRegion | undefined => props.regions[regionIndex.value]);
const rowCount = computed(() => (region.value ? Math.ceil(region.value.size / COLUMNS) : 0));
const root = ref<HTMLElement | null>(null);
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
const column = ref<"hex" | "ascii">("hex"); // colonne où l'on saisit : chiffres hexadécimaux ou caractères
const error = ref("");

/** Observe en écriture la plage sélectionnée (le détail se règle dans « Points d'observation »). */
async function watchSelection() {
  const s = selection.value;
  if (!s) return;
  const failure = await addWatch(emulator, s.start, s.end - s.start + 1, "write", false);
  if (failure) error.value = failure;
  else notify(t("hex.watching", { address: hex0x(s.start) }));
}

const firstRow = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW)));
const baseAddress = computed(() => (region.value?.base ?? 0) + firstRow.value * COLUMNS);

// ---- lecture --------------------------------------------------------------------------------------

const bytes = ref<number[]>([]);
const previous = ref<number[] & { base?: number }>([]);
// Les octets qui viennent de changer restent surlignés un instant : à 8 relevés par seconde, un simple « a changé depuis le
// relevé précédent » ne se verrait pas.
const changed = reactive(new Map<number, number>());

let readSeq = 0;
async function readWindow() {
  const r = region.value;
  if (!r) return;
  const rows = Math.min(viewRows.value + 1, rowCount.value - firstRow.value);
  const length = Math.max(0, Math.min(rows * COLUMNS, r.base + r.size - baseAddress.value));
  if (length === 0) return void (bytes.value = []);
  const seq = ++readSeq;
  const at = baseAddress.value;
  const reply = await fetchBytes(at, length);
  if (seq !== readSeq) return; // une lecture plus récente est partie entre-temps
  if (!reply.ok) {
    error.value = reply.error;
    bytes.value = [];
    return;
  }
  error.value = "";
  const next = reply.bytes;
  // Les octets qui ont changé depuis la lecture précédente de la même fenêtre sont mis en évidence.
  const now = performance.now();
  for (const [address, when] of changed) if (now - when > HIGHLIGHT_MS) changed.delete(address);
  if (previous.value.length === next.length && previous.value.base === at) {
    next.forEach((b, i) => b !== previous.value[i] && changed.set(at + i, now));
  }
  previous.value = Object.assign(next, { base: at });
  bytes.value = next;
}

// La fenêtre visible est relue plusieurs fois par seconde : la mémoire s'affiche (et se modifie) en temps réel.
const REFRESH_MS = 120;
const HIGHLIGHT_MS = 1200;
let stopPolling = () => {};
onMounted(() => {
  measure();
  stopPolling = poll(readWindow, REFRESH_MS);
  addEventListener("resize", measure);
});
onBeforeUnmount(() => {
  stopPolling();
  removeEventListener("resize", measure);
});
watch([baseAddress, regionIndex, () => state.revision, () => props.version], readWindow);
watch(() => props.regions.length, () => {
  if (regionIndex.value >= props.regions.length) regionIndex.value = 0;
  readWindow();
  // Une demande d'adresse arrivée avant que les zones soient connues (l'onglet vient de s'ouvrir) est reprise maintenant.
  if (ui.hexGoto && ui.hexGoto.seq !== handledSeq && !props.read) void followRequest(ui.hexGoto);
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

// Une demande venue d'ailleurs (clic sur une adresse de la recherche, de la carte…) : on s'y rend, puis on ramène l'éditeur
// dans la fenêtre : la page remonte vers lui, où qu'on ait cliqué.
let handledSeq = 0;
async function followRequest(request: { address: number; seq: number }) {
  if (!(await goTo(request.address))) return;
  handledSeq = request.seq;
  root.value?.scrollIntoView({ behavior: "smooth", block: "start" });
}

async function goTo(address: number, select = true): Promise<boolean> {
  const index = regionOf(address);
  if (index < 0) {
    error.value = t("hex.unmapped", { address: hex0x(address) });
    return false;
  }
  error.value = "";
  // Le curseur est posé avant le changement de zone : sinon l'observateur de la zone, qui remet la fenêtre en haut, passerait
  // après le défilement qui suit et annulerait le déplacement (c'est ce qui obligeait à cliquer deux fois).
  if (select) {
    cursor.value = anchor.value = address;
    pending.value = "";
  }
  regionIndex.value = index;
  await nextTick();
  const row = Math.floor((address - (props.regions[index]?.base ?? 0)) / COLUMNS);
  if (scroller.value) scroller.value.scrollTop = Math.max(0, row * ROW - ROW * 3);
  onScroll();
  await readWindow();
  return true;
}

function submitGoto() {
  const address = parseAddress(gotoText.value);
  if (address === undefined) error.value = t("hex.badAddress");
  else goTo(address);
}

watch(
  () => ui.hexGoto,
  (request) => request && !props.read && followRequest(request),
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
function pointerDown(address: number, event: PointerEvent, where: "hex" | "ascii" = "hex") {
  column.value = where;
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

async function write(address: number, data: number[]): Promise<boolean> {
  if (!canEdit.value) return false;
  if (props.write) {
    if (!props.write(address, data)) return false;
  } else {
    const reply = await call({ cmd: "memory_write", address, hex: data.map((b: number) => hex(b, 2)).join("") });
    if (!reply.ok) return false;
    await emulator.bump();
  }
  await readWindow();
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
  // Dans la colonne ASCII, chaque caractère tapé remplace l'octet sous le curseur (les lettres a à f ne sont pas des chiffres).
  if (column.value === "ascii" && event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && cursor.value !== null) {
    event.preventDefault();
    if (!canEdit.value) return;
    const code = event.key.charCodeAt(0);
    if (code > 255) return void (error.value = t("hex.badChar", { char: event.key }));
    // Le curseur avance tout de suite : on peut taper plus vite que le module n'écrit (l'écriture est asynchrone).
    const at = cursor.value;
    move(1, false);
    void write(at, [code]);
    return;
  }
  if (/^[0-9a-f]$/i.test(event.key) && !event.ctrlKey && !event.metaKey && !event.altKey && cursor.value !== null) {
    event.preventDefault();
    if (!canEdit.value) return;
    if (!pending.value) {
      pending.value = event.key;
    } else {
      const value = parseInt(pending.value + event.key, 16);
      pending.value = "";
      const at = cursor.value;
      move(1, false);
      void write(at, [value]);
    }
  }
}

async function paste(event: ClipboardEvent) {
  if (cursor.value === null || !canEdit.value) return;
  const text = (event.clipboardData?.getData("text") ?? "").replace(/0x/gi, "").replace(/[^0-9a-f]/gi, "");
  if (text.length < 2) return;
  event.preventDefault();
  const data = (text.match(/../g) ?? []).map((b) => parseInt(b, 16));
  if (data.length > 1 && !(await confirmDialog(t("hex.pasteConfirm", { count: data.length, address: hex0x(cursor.value) })))) return;
  write(cursor.value, data);
}

// ---- actions --------------------------------------------------------------------------------------

async function selectedBytes(): Promise<number[] | null> {
  const s = selection.value;
  if (!s || s.end - s.start > 1 << 20) return null;
  const reply = await fetchBytes(s.start, s.end - s.start + 1);
  return reply.ok ? reply.bytes : null;
}

async function copy(kind: "hex" | "ascii") {
  const data = await selectedBytes();
  if (!data) return;
  const text = kind === "ascii" ? data.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : ".")).join("") : data.map((b) => hex(b, 2)).join(" ");
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    error.value = t("hex.copyFailed");
  }
}

// Écrire une valeur : une seule fenêtre, deux usages (voir `HexValueDialog`).
interface ValueRequest {
  mode: "fill" | "edit";
  address: number;
  length: number;
  type: ScanType;
  base: Base;
  text: string;
}
const valueDialog = ref<ValueRequest | null>(null);
/** Fenêtre d'export : plage proposée (la sélection, sinon 256 octets au curseur). */
const exportDialog = ref<{ address: number; length: number } | null>(null);
function openExport() {
  const s = selection.value;
  const at = s?.start ?? cursor.value ?? region.value?.base ?? 0;
  exportDialog.value = { address: at, length: s ? s.end - s.start + 1 : 256 };
}

/** « Remplir » : répète une valeur sur toute la sélection. */
function openFill() {
  const s = selection.value;
  if (s) valueDialog.value = { mode: "fill", address: s.start, length: s.end - s.start + 1, type: "u8", base: "hex", text: "00" };
}

/** Clic dans le tableau des valeurs : modifier cette valeur à partir du premier octet sélectionné (pas du curseur). */
function openEdit(column: Column, base: Base) {
  const sel = selection.value;
  const shown = base === "hex" ? column.hex : column.dec;
  if (!sel || shown === NONE) return;
  const type = (SCAN_TYPES as readonly string[]).includes(column.id) ? (column.id as ScanType) : "u8";
  valueDialog.value = { mode: "edit", address: sel.start, length: SIZE[type], type, base: column.id === "bin" || column.id === "chr" ? "dec" : base, text: column.id === "bin" ? String(parseInt(column.dec, 2)) : column.id === "chr" ? String(column.dec.charCodeAt(0)) : shown };
}

async function applyValue(bytes: Uint8Array) {
  const request = valueDialog.value;
  if (!request) return;
  valueDialog.value = null;
  const total = request.mode === "fill" ? request.length : bytes.length;
  const data = request.mode === "fill" ? tile(bytes, total) : bytes;
  const CHUNK = 1 << 16;
  for (let at = 0; at < data.length; at += CHUNK) {
    if (!(await write(request.address + at, Array.from(data.subarray(at, at + CHUNK))))) {
      error.value = t("hex.fillFailed", { address: hex0x(request.address + at) });
      return;
    }
  }
  error.value = "";
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
    const reply = await fetchBytes(start, length);
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

// Inspecteur : les valeurs que prennent les octets à partir du PREMIER octet sélectionné (pas du dernier), pour chaque type,
// en tableau. La colonne dont la taille est celle de la sélection est la valeur exacte de la zone choisie.
interface Column {
  id: string;
  label: string;
  size: number;
  /** Valeur en décimal et en hexadécimal ; « — » quand les octets manquent. */
  dec: string;
  hex: string;
}
interface Inspected {
  address: number;
  /** Nombre d'octets sélectionnés. */
  length: number;
  columns: Column[];
}
const inspector = ref<Inspected | null>(null);
let inspectSeq = 0;
const NONE = "—";
async function inspect() {
  const seq = ++inspectSeq;
  const sel = selection.value;
  const r = region.value;
  if (!sel || !r) return void (inspector.value = null);
  const reply = await fetchBytes(sel.start, Math.min(8, r.base + r.size - sel.start));
  if (seq !== inspectSeq) return;
  if (!reply.ok || reply.bytes.length === 0) return void (inspector.value = null);
  const data = new Uint8Array(reply.bytes);
  const view = new DataView(data.buffer);
  const le = !bigEndian.value;
  const has = (n: number) => data.length >= n;
  const pad = (v: number, digits: number) => "0x" + hex(v >>> 0, digits);
  const float = (v: number) => String(Number(v.toPrecision(9)));
  const column = (id: string, size: number, dec: () => string, hexa: () => string): Column => (has(size) ? { id, label: id, size, dec: dec(), hex: hexa() } : { id, label: id, size, dec: NONE, hex: NONE });
  const columns: Column[] = [
    column("u8", 1, () => String(view.getUint8(0)), () => pad(view.getUint8(0), 2)),
    column("i8", 1, () => String(view.getInt8(0)), () => pad(view.getUint8(0), 2)),
    column("u16", 2, () => String(view.getUint16(0, le)), () => pad(view.getUint16(0, le), 4)),
    column("i16", 2, () => String(view.getInt16(0, le)), () => pad(view.getUint16(0, le), 4)),
    column("u32", 4, () => String(view.getUint32(0, le)), () => pad(view.getUint32(0, le), 8)),
    column("i32", 4, () => String(view.getInt32(0, le)), () => pad(view.getUint32(0, le), 8)),
    column("f32", 4, () => float(view.getFloat32(0, le)), () => pad(view.getUint32(0, le), 8)),
    column("f64", 8, () => float(view.getFloat64(0, le)), () => "0x" + hex(view.getUint32(le ? 4 : 0, le), 8) + hex(view.getUint32(le ? 0 : 4, le), 8)),
    { id: "bin", label: t("hex.binary"), size: 0, dec: view.getUint8(0).toString(2).padStart(8, "0"), hex: NONE },
    { id: "chr", label: t("hex.character"), size: 0, dec: printable(view.getUint8(0)), hex: NONE },
  ];
  inspector.value = { address: sel.start, length: sel.end - sel.start + 1, columns };
}
watch([cursor, anchor, bytes, bigEndian, () => state.revision], inspect);
const exact = (c: Column, length: number) => c.size === length;
const beyond = (c: Column, length: number) => c.size > length;
</script>

<template>
  <section ref="root" class="hex" :aria-label="t('hex.label')">
    <div class="toolbar" role="toolbar" :aria-label="t('hex.toolbar')">
      <label v-if="regions.length > 1" class="field">{{ t("hex.region") }}
        <select v-model="regionIndex" v-memo="[regionIndex, regions.length]">
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
        <select v-model.number="group" v-memo="[group]"><option :value="1">1</option><option :value="2">2</option><option :value="4">4</option><option :value="8">8</option></select>
      </label>
    </div>
    <div class="actions">
      <button :disabled="!selection" @click="copy('hex')">{{ t("hex.copyHex") }}</button>
      <button :disabled="!selection" @click="copy('ascii')">{{ t("hex.copyAscii") }}</button>
      <button :disabled="!selection || !canEdit" @click="openFill">{{ t("hex.fill") }}</button>
      <button :disabled="!selection" :title="t('hex.watchHint')" @click="watchSelection">{{ t("hex.watch") }}</button>
      <button :title="t('hex.exportHint')" @click="openExport">{{ t("hex.export") }}</button>
      <span v-if="selection" class="muted mono">{{ hex0x(selection.start) }}{{ selection.end > selection.start ? ` → ${hex0x(selection.end)} (${selection.end - selection.start + 1})` : "" }}</span>
      <span class="spacer" />
      <span v-if="findMessage" class="muted">{{ findMessage }}</span>
    </div>
    <p v-if="error" class="error">{{ error }}</p>

    <div class="inspector" role="group" :aria-label="t('hex.inspector')">
      <div class="inspector-head">
        <span v-if="inspector"><strong>{{ t("hex.inspectAt", { address: hex0x(inspector.address) }) }}</strong>
          <span class="muted"> · {{ t("hex.selected", { n: inspector.length }, inspector.length) }}</span></span>
        <span v-else class="muted">{{ t("hex.inspectNone") }}</span>
        <label class="endian"><input v-model="bigEndian" type="checkbox" data-keep-keys /> {{ t("hex.bigEndian") }}</label>
      </div>
      <div v-if="inspector" class="inspector-scroll">
        <table class="mono">
          <thead>
            <tr>
              <th />
              <th v-for="c in inspector.columns" :key="c.id" :class="{ exact: exact(c, inspector.length), over: beyond(c, inspector.length) }">{{ c.label }}</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th scope="row">{{ t("hex.decimal") }}</th>
              <td v-for="c in inspector.columns" :key="c.id" class="edit" :class="{ exact: exact(c, inspector.length), over: beyond(c, inspector.length) }" :title="canEdit ? t('hex.clickToEdit') : c.dec" :tabindex="canEdit && c.dec !== NONE ? 0 : -1" @click="canEdit && openEdit(c, 'dec')" @keydown.enter="canEdit && openEdit(c, 'dec')">{{ c.dec }}</td>
            </tr>
            <tr>
              <th scope="row">{{ t("hex.hexadecimal") }}</th>
              <td v-for="c in inspector.columns" :key="c.id" class="edit" :class="{ exact: exact(c, inspector.length), over: beyond(c, inspector.length) }" :title="canEdit ? t('hex.clickToEdit') : c.hex" :tabindex="canEdit && c.hex !== NONE ? 0 : -1" @click="canEdit && openEdit(c, 'hex')" @keydown.enter="canEdit && openEdit(c, 'hex')">{{ c.hex }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

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
                  :class="['cell', gap(j), { sel: inSelection(cell.address), cur: cell.address === cursor && column === 'hex', mark: cell.address === cursor && column === 'ascii', chg: changed.has(cell.address), zero: cell.value === 0 }]"
                  @pointerdown.prevent="pointerDown(cell.address, $event, 'hex')"
                  @pointerenter="pointerEnter(cell.address)"
                >{{ cell.address === cursor && pending ? pending + "_" : hex(cell.value ?? 0, 2) }}</span>
              </span>
              <span class="ascii">
                <span
                  v-for="cell in row.cells"
                  :key="cell.address"
                  :class="['ch', { sel: inSelection(cell.address), cur: cell.address === cursor && column === 'ascii', mark: cell.address === cursor && column === 'hex', chg: changed.has(cell.address) }]"
                  @pointerdown.prevent="pointerDown(cell.address, $event, 'ascii')"
                  @pointerenter="pointerEnter(cell.address)"
                >{{ printable(cell.value) }}</span>
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <p class="muted hint">{{ t("hex.help") }}</p>
    <ExportDialog v-if="exportDialog" v-bind="exportDialog" @close="exportDialog = null" />
    <HexValueDialog v-if="valueDialog" v-bind="valueDialog" :big-endian="bigEndian" @close="valueDialog = null" @apply="applyValue" />
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
/* Le même octet dans l'autre colonne : un simple contour, pour voir où l'on est sans confondre avec la colonne de saisie. */
.cell.mark, .ch.mark { outline: 1px solid var(--accent); outline-offset: -1px; }
.cell.chg, .ch.chg { color: var(--warn); font-weight: 700; }
.ascii { display: inline-flex; flex: none; width: 16ch; }
.ch { width: 1ch; }
.inspector { border: 1px solid var(--line); border-radius: 6px; background: var(--surface); overflow: hidden; }
.inspector-head { display: flex; flex-wrap: wrap; gap: 4px 12px; align-items: center; justify-content: space-between; padding: 6px 10px; background: var(--surface-2); border-bottom: 1px solid var(--line); font-size: 12.5px; }
.endian { font-family: system-ui, sans-serif; font-size: 12px; }
.inspector-scroll { overflow-x: auto; }
.inspector table { border-collapse: collapse; width: 100%; font-size: 12px; }
.inspector th, .inspector td { padding: 3px 7px; text-align: right; white-space: nowrap; max-width: 13ch; overflow: hidden; text-overflow: ellipsis; border-bottom: 1px solid var(--line); }
.inspector tbody tr:last-child th, .inspector tbody tr:last-child td { border-bottom: none; }
.inspector thead th { color: var(--muted); font-weight: 600; background: var(--surface-2); }
.inspector th[scope="row"] { text-align: left; color: var(--muted); font-weight: 500; font-family: system-ui, sans-serif; }
/* La colonne qui a la taille de la sélection : sa valeur est celle de la zone choisie ; celles qui dépassent la sélection sont atténuées. */
.inspector .exact { background: var(--accent-soft); color: var(--text); }
.inspector .over { color: var(--muted); }
.inspector td.edit { cursor: pointer; }
.inspector td.edit:hover { outline: 1px solid var(--accent); outline-offset: -1px; }
.hint { margin: 0; font-size: 12px; }
</style>
