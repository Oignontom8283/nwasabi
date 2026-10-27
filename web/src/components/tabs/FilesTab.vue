<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { fromBase64, toBase64 } from "@nwasabi/js";
import { confirmDialog } from "../../dialogs.js";
import { download, formatBytes, poll, readBytes } from "../../util.js";
import ContextMenu from "../ContextMenu.vue";
import type { MenuItem } from "../ContextMenu.vue";
import type { Reply } from "@nwasabi/js";
import HexEditor from "../HexEditor.vue";
import Icon from "../Icon.vue";

const { t, locale } = useI18n();
const emulator = useEmu();
const { state, call, quiet } = emulator;

interface FsFile {
  name: string;
  /** "python", "system" ou "other" (le type précis après les deux-points est ignoré). */
  kind: string;
  size: number;
}

const files = ref<FsFile[]>([]);
const stats = ref<Reply | null>(null);
const corrupt = ref<number | null>(null);
const selected = ref("");
// Sélection multiple : `marked` (Ctrl ou Cmd + clic, Maj + clic, rectangle dans l'espace libre) ; `selected` est le fichier ouvert dans l'éditeur.
const marked = ref(new Set<string>());
let anchor = "";
const message = ref("");
const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);
const importInput = ref<HTMLInputElement | null>(null);
const nfsInput = ref<HTMLInputElement | null>(null);
let lastGeneration = -1;

// L'éditeur : texte d'un script, ou aperçu d'un fichier opaque.
const text = ref("");
const original = ref("");
const autoImport = ref(false);
const originalAuto = ref(false);
// Un fichier qui n'est pas un script se modifie en hexadécimal, sur une copie des octets : « Enregistrer » les écrit.
const buffer = shallowRef<Uint8Array>(new Uint8Array());
const originalBuffer = shallowRef<Uint8Array>(new Uint8Array());
const bufferVersion = ref(0);
const bufferChanged = computed(() => {
  void bufferVersion.value;
  const a = buffer.value;
  const b = originalBuffer.value;
  return a.length !== b.length || a.some((byte, i) => byte !== b[i]);
});
const hexRegions = computed(() => [{ name: selected.value, base: 0, size: buffer.value.length }]);
const hexRead = (address: number, length: number) => Array.from(buffer.value.subarray(address, address + length));
function hexWrite(address: number, data: number[]) {
  if (address < 0 || address + data.length > buffer.value.length) return false;
  buffer.value.set(data, address);
  bufferVersion.value++;
  return true;
}
const isText = computed(() => current.value?.kind === "python");
const dirty = computed(() => (isText.value ? text.value !== original.value || autoImport.value !== originalAuto.value : bufferChanged.value));
const current = computed(() => files.value.find((f) => f.name === selected.value));

const paused = computed(() => !state.running);
const usedPercent = computed(() => (stats.value ? Math.round((stats.value.used / stats.value.total) * 100) : 0));

async function refresh(force = false) {
  const list = await quiet({ cmd: "fs_list" });
  if (!list.ok) return;
  if (!force && list.generation === lastGeneration) return;
  lastGeneration = list.generation;
  files.value = list.files;
  corrupt.value = list.corrupt_at;
  const s = await quiet({ cmd: "fs_stats" });
  stats.value = s.ok ? s : null;
  if (selected.value && !(list.files as FsFile[]).some((f) => f.name === selected.value)) closeEditor();
  marked.value = new Set([...marked.value].filter((name) => (list.files as FsFile[]).some((f) => f.name === name)));
}

async function fail(reply: Reply): Promise<boolean> {
  message.value = reply.ok ? "" : (reply.error ?? "");
  state.error = "";
  await refresh(true);
  return reply.ok;
}

function closeEditor() {
  selected.value = "";
  text.value = original.value = "";
  buffer.value = originalBuffer.value = new Uint8Array();
}

async function select(file: FsFile) {
  marked.value = new Set([file.name]);
  anchor = file.name;
  if (dirty.value && !(await confirmDialog(t("files.discard"), { accept: t("files.discardAccept") }))) return;
  selected.value = file.name;
  if (file.kind === "python") {
    const src = await call({ cmd: "fs_read", name: file.name, python: true });
    text.value = original.value = src.text ?? "";
    autoImport.value = originalAuto.value = !!src.auto_import;
  } else {
    text.value = original.value = "";
    const reply = await call({ cmd: "fs_read", name: file.name });
    const bytes = reply.ok ? fromBase64(reply.content) : new Uint8Array();
    buffer.value = bytes;
    originalBuffer.value = bytes.slice();
    bufferVersion.value++;
  }
}

async function save() {
  if (!current.value) return;
  if (!isText.value) {
    if (await fail(await call({ cmd: "fs_write", name: selected.value, content: toBase64(buffer.value) }))) originalBuffer.value = buffer.value.slice();
    return;
  }
  if (await fail(await call({ cmd: "fs_write", name: selected.value, python: true, text: text.value, auto_import: autoImport.value }))) {
    original.value = text.value;
    originalAuto.value = autoImport.value;
  }
}

// Créer et renommer se font dans la liste même : une ligne de saisie à la place du nom. Entrée ou quitter la ligne
// valide, Échap annule ; laisser le nom proposé le garde tel quel.
const editing = ref<{ mode: "new" | "rename"; name: string; value: string } | null>(null);
let cancelled = false;

function create() {
  editing.value = { mode: "new", name: "", value: "script.py" };
}

function startRename(file: FsFile) {
  select(file);
  editing.value = { mode: "rename", name: file.name, value: file.name };
}

// Vue rappelle la fonction de référence à chaque rendu : on ne sélectionne le nom qu'à la première fois.
const focused = new WeakSet<HTMLInputElement>();
function focusName(target: unknown) {
  const el = target as HTMLInputElement | null;
  if (!el || focused.has(el)) return;
  focused.add(el);
  nextTick(() => {
    el.focus();
    const dot = el.value.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
  });
}

async function endEdit(commit: boolean) {
  const edit = editing.value;
  if (!edit) return;
  editing.value = null;
  if (!commit) return;
  const name = edit.value.trim();
  if (!name || name === edit.name) return;
  if (edit.mode === "new") {
    const python = name.endsWith(".py");
    const request = python
      ? { cmd: "fs_write", name, python: true, text: "", auto_import: false }
      : { cmd: "fs_write", name, text: "" };
    if (await fail(await call(request))) select({ name, kind: python ? "python" : "other", size: 0 });
  } else if ((await fail(await call({ cmd: "fs_rename", from: edit.name, to: name }))) && selected.value === edit.name) {
    selected.value = name;
  }
}

function editKey(event: KeyboardEvent) {
  event.stopPropagation(); // ni Suppr ni F2 ni les flèches ne doivent agir sur la ligne pendant la saisie
  if (event.key === "Enter") (event.target as HTMLInputElement).blur();
  else if (event.key === "Escape") {
    cancelled = true;
    (event.target as HTMLInputElement).blur();
  } else return;
  event.preventDefault();
}

function editBlur() {
  const abort = cancelled;
  cancelled = false;
  endEdit(!abort);
}

const targets = (file: FsFile) => (marked.value.has(file.name) ? files.value.filter((f) => marked.value.has(f.name)) : [file]);

/** Supprime des fichiers ; Maj enfoncé : sans demander confirmation. */
async function remove(list: FsFile[], skipConfirm = false) {
  if (!list.length) return;
  const question = list.length === 1 ? t("files.confirmDelete", { name: list[0]?.name ?? "" }) : t("files.confirmDeleteMany", { n: list.length });
  if (!skipConfirm && !(await confirmDialog(question, { danger: true, accept: t("files.delete") }))) return;
  for (const file of list) {
    if ((await fail(await call({ cmd: "fs_remove", name: file.name }))) && selected.value === file.name) closeEditor();
  }
}

async function clearAll() {
  if (!files.value.length) return;
  if (!(await confirmDialog(t("files.confirmClear", { n: files.value.length }), { danger: true, accept: t("files.clearAll"), title: t("files.clearAll") }))) return;
  await remove([...files.value], true);
}

function rowClick(event: MouseEvent, file: FsFile) {
  if (event.ctrlKey || event.metaKey) {
    const next = new Set(marked.value);
    if (!next.delete(file.name)) next.add(file.name);
    marked.value = next;
    anchor = file.name;
  } else if (event.shiftKey && anchor) {
    const names = files.value.map((f) => f.name);
    const [from = 0, to = 0] = [names.indexOf(anchor), names.indexOf(file.name)].sort((x, y) => x - y);
    marked.value = new Set(names.slice(from, to + 1));
  } else select(file);
}

// Rectangle de sélection dans l'espace libre sous la liste (il se dessine par-dessus, sans changer la mise en page).
const rowsBox = ref<HTMLElement | null>(null);
const band = ref<{ x: number; y: number; w: number; h: number } | null>(null); // relatif au conteneur
let bandStart: { x: number; y: number } | null = null;
let bandBase = new Set<string>();
function bandDown(event: PointerEvent) {
  const area = rowsBox.value;
  if (!area || event.target !== area || event.button !== 0) return;
  const box = area.getBoundingClientRect();
  bandStart = { x: event.clientX - box.left + area.scrollLeft, y: event.clientY - box.top + area.scrollTop };
  bandBase = event.ctrlKey || event.metaKey || event.shiftKey ? new Set(marked.value) : new Set();
  if (!bandBase.size) marked.value = new Set();
  area.setPointerCapture(event.pointerId);
}
function bandMove(event: PointerEvent) {
  const area = rowsBox.value;
  if (!area || !bandStart) return;
  const box = area.getBoundingClientRect();
  const x = event.clientX - box.left + area.scrollLeft;
  const y = event.clientY - box.top + area.scrollTop;
  const rect = { x: Math.min(bandStart.x, x), y: Math.min(bandStart.y, y), w: Math.abs(x - bandStart.x), h: Math.abs(y - bandStart.y) };
  band.value = rect;
  const next = new Set(bandBase);
  for (const row of area.querySelectorAll<HTMLElement>("[data-file]")) {
    const top = row.offsetTop;
    if (top < rect.y + rect.h && top + row.offsetHeight > rect.y && rect.x < area.clientWidth && row.dataset.file) next.add(row.dataset.file);
  }
  marked.value = next;
}
function bandUp() {
  bandStart = null;
  band.value = null;
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* le presse-papiers peut être refusé */
  }
}

async function downloadFile(file: FsFile) {
  const reply = await call({ cmd: "fs_read", name: file.name });
  if (reply.ok) download(file.name, fromBase64(reply.content) as Uint8Array<ArrayBuffer>);
}

async function importFiles(event: Event) {
  const input = event.target as HTMLInputElement;
  for (const file of input.files ?? []) {
    await fail(await call({ cmd: "fs_write", name: file.name, content: toBase64(await readBytes(file)) }));
  }
  input.value = "";
}

async function exportNfs() {
  const reply = await call({ cmd: "fs_export" });
  if (await fail(reply)) download("files.nfs", JSON.stringify(reply.nfs, null, 1), "application/json");
}

async function importNfs(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  let document: unknown;
  try {
    document = JSON.parse(await file.text());
  } catch {
    message.value = t("errors.json", { source: file.name });
    return;
  }
  const replace = await confirmDialog(t("files.importMode", { name: file.name }), { accept: t("files.replaceAll"), title: t("files.importNfs") });
  if (await fail(await call({ cmd: "fs_import", nfs: document, replace }))) await emulator.bump();
}

function openMenu(event: { preventDefault(): void; clientX: number; clientY: number }, file: FsFile) {
  event.preventDefault();
  if (!marked.value.has(file.name)) select(file);
  const many = marked.value.has(file.name) && marked.value.size > 1 ? marked.value.size : 0;
  menu.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      { label: t("files.edit"), action: () => select(file), disabled: file.kind !== "python" },
      { label: t("files.rename"), action: () => startRename(file) },
      { label: t("files.download"), action: () => downloadFile(file) },
      { label: t("files.copySize"), action: () => copy(String(file.size)) },
      { label: t("files.copyName"), action: () => copy(file.name) },
      { separator: true },
      { label: many ? t("files.deleteMany", { n: many }) : t("files.delete"), action: (event) => remove(targets(file), event?.shiftKey), danger: true },
    ],
  };
}

function rowKey(event: KeyboardEvent, file: FsFile, index: number) {
  const go = (i: number) => document.getElementById(`file-${files.value[i]?.name}`)?.focus();
  if (event.key === "ArrowDown") go(Math.min(index + 1, files.value.length - 1));
  else if (event.key === "ArrowUp") go(Math.max(index - 1, 0));
  else if (event.key === "Enter") select(file);
  else if (event.key === "F2") startRename(file);
  else if (event.key === "Delete") remove(targets(file), event.shiftKey);
  else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") marked.value = new Set(files.value.map((f) => f.name));
  else if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
    const box = (event.currentTarget as Element).getBoundingClientRect();
    openMenu({ preventDefault() {}, clientX: box.left + 30, clientY: box.bottom }, file);
  } else return;
  event.preventDefault();
}

const ICON = (f: FsFile) => (f.kind === "python" ? "python" : f.kind === "system" ? "gear" : "file");

let stopPolling = () => {};
onMounted(() => {
  // L'application modifie le stockage en direct : on interroge la « génération » régulièrement.
  stopPolling = poll(() => refresh(), 400);
});
onBeforeUnmount(() => stopPolling());
watch(() => state.revision, () => refresh(true));
</script>

<template>
  <div class="files">
    <div class="toolbar" role="toolbar" :aria-label="t('files.toolbar')">
      <button class="flat" :title="t('files.new')" @click="create"><Icon name="plus" /> {{ t("files.new") }}</button>
      <button class="flat" :title="t('files.import')" @click="importInput?.click()"><Icon name="upload" /> {{ t("files.import") }}</button>
      <span class="sep" />
      <button class="flat" :title="t('files.importNfsHint')" @click="nfsInput?.click()">{{ t("files.importNfs") }}</button>
      <button class="flat" :disabled="!files.length" :title="t('files.clearAllHint')" @click="clearAll">{{ t("files.clearAll") }}</button>
      <button class="flat" :disabled="!paused" :title="paused ? t('files.exportNfsHint') : t('common.pauseRequired')" @click="exportNfs">{{ t("files.exportNfs") }}</button>
      <input ref="importInput" type="file" multiple hidden @change="importFiles" />
      <input ref="nfsInput" type="file" accept=".nfs,application/json" hidden @change="importNfs" />
    </div>
    <p v-if="corrupt !== null" class="error">{{ t("files.corrupt", { offset: corrupt }) }}</p>
    <p v-if="message" class="error" role="alert">{{ message }}</p>

    <div class="explorer" role="listbox" :aria-label="t('files.list')">
      <div class="head" aria-hidden="true"><span /><span>{{ t("files.colName") }}</span><span>{{ t("files.colType") }}</span><span class="r">{{ t("files.colSize") }}</span></div>
      <div ref="rowsBox" class="rows" @pointerdown="bandDown" @pointermove="bandMove" @pointerup="bandUp" @pointercancel="bandUp">
        <div v-if="editing?.mode === 'new'" class="row">
          <span class="ico" aria-hidden="true"><Icon name="file" :size="14" /></span>
          <input :ref="focusName" v-model="editing.value" class="name-input" spellcheck="false" :aria-label="t('files.newName')" data-keep-keys @keydown="editKey" @blur="editBlur" />
          <span class="type muted">{{ t("files.new") }}</span><span />
        </div>
        <div
          v-for="(f, i) in files"
          :id="`file-${f.name}`"
          :key="f.name"
          class="row"
          :class="{ sel: marked.has(f.name) }"
          :data-file="f.name"
          role="option"
          tabindex="0"
          :aria-selected="f.name === selected"
          @click="rowClick($event, f)"
          @dblclick="select(f)"
          @contextmenu="openMenu($event, f)"
          @keydown="rowKey($event, f, i)"
        >
          <span class="ico" aria-hidden="true"><Icon :name="ICON(f)" :size="14" /></span>
          <input
            v-if="editing?.mode === 'rename' && editing.name === f.name"
            :ref="focusName"
            v-model="editing.value"
            class="name-input"
            spellcheck="false"
            :aria-label="t('files.renameTo')"
            data-keep-keys
            @click.stop
            @keydown="editKey"
            @blur="editBlur"
          />
          <span v-else class="name">{{ f.name }}</span>
          <span class="type muted">{{ t(`files.kinds.${f.kind.split(":")[0]}`) }}</span>
          <span class="size mono muted r">{{ formatBytes(f.size, locale) }}</span>
        </div>
        <div v-if="band" class="band" :style="{ left: band.x + 'px', top: band.y + 'px', width: band.w + 'px', height: band.h + 'px' }" />
        <p v-if="!files.length && !editing" class="empty muted">{{ t("files.empty") }}</p>
      </div>
      <div class="status" :title="t('files.storageHint')">
        <div class="bar" role="progressbar" :aria-valuenow="usedPercent" aria-valuemin="0" aria-valuemax="100" :aria-label="t('files.storage')"><div :style="{ width: usedPercent + '%' }" :class="{ high: usedPercent > 85 }" /></div>
        <span v-if="stats" class="muted">{{ t("files.count", stats.records) }} · {{ t("files.used", { used: formatBytes(stats.used, locale), total: formatBytes(stats.total, locale), percent: usedPercent }) }}</span>
      </div>
    </div>
    <p class="muted hint">{{ t("files.hint") }}</p>

    <section v-if="selected && current" class="editor">
      <header>
        <span class="ico"><Icon :name="ICON(current)" :size="14" /></span><strong>{{ selected }}</strong>
        <small class="kind">{{ isText ? t("files.formatText") : current.kind === "system" ? t("files.formatSystem") : t("files.formatBinary") }}</small>
        <span v-if="dirty" class="muted">● {{ t("files.modified") }}</span>
        <span class="spacer" />
        <label v-if="isText" class="auto"><input v-model="autoImport" type="checkbox" /> {{ t("files.autoImport") }}</label>
        <button class="primary" :disabled="!dirty" title="Ctrl+S" @click="save">{{ t("files.save") }}</button>
        <button :aria-label="t('common.close')" class="flat icon" @click="closeEditor"><Icon name="close" :size="14" /></button>
      </header>
      <textarea
        v-if="isText"
        v-model="text"
        class="mono"
        rows="12"
        spellcheck="false"
        data-keep-keys
        :aria-label="selected"
        @keydown.ctrl.s.prevent="save"
        @keydown.meta.s.prevent="save"
      />
      <HexEditor v-else :regions="hexRegions" :read="hexRead" :write="hexWrite" :editable="true" :version="bufferVersion" :height="220" />
    </section>

    <ContextMenu v-if="menu" :items="menu.items" :x="menu.x" :y="menu.y" @close="menu = null" />
  </div>
</template>

<style scoped>
.files { display: grid; gap: 8px; }
.toolbar { display: flex; flex-wrap: wrap; gap: 2px; align-items: center; }
.sep { width: 1px; height: 18px; background: var(--line); margin: 0 4px; }
.explorer { border: 1px solid var(--line); border-radius: 6px; background: var(--surface); overflow: hidden; display: grid; grid-template-rows: auto minmax(110px, 1fr) auto; }
.head, .row { display: grid; grid-template-columns: 20px 1fr 70px 78px; gap: 6px; align-items: center; padding: 0 10px; }
.head { height: 24px; font-size: 12px; color: var(--muted); border-bottom: 1px solid var(--line); background: var(--surface-2); }
.rows { max-height: 260px; overflow: auto; position: relative; user-select: none; }
.band { position: absolute; pointer-events: none; background: color-mix(in srgb, var(--accent) 18%, transparent); border: 1px solid var(--accent); }
.row { height: 25px; cursor: default; outline-offset: -2px; }
.row:hover { background: var(--accent-soft); }
.row.sel { background: color-mix(in srgb, var(--accent) 24%, transparent); }
.row .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row .type, .row .size { font-size: 12px; }
.r { text-align: right; }
.ico { display: inline-flex; color: var(--muted); }
.empty { margin: 0; padding: 18px; text-align: center; pointer-events: none; }
.status { display: flex; align-items: center; gap: 10px; padding: 3px 10px; border-top: 1px solid var(--line); background: var(--surface-2); font-size: 12px; }
.bar { width: 80px; height: 6px; background: var(--line); border-radius: 3px; overflow: hidden; flex: none; }
.bar div { height: 100%; background: var(--accent); }
.bar div.high { background: var(--danger); }
.hint { margin: 0; font-size: 12px; }
.editor { border: 1px solid var(--line); border-radius: 6px; overflow: hidden; }
.editor header { display: flex; gap: 6px; align-items: center; padding: 3px 6px 3px 10px; background: var(--surface-2); border-bottom: 1px solid var(--line); }
.spacer { flex: 1; }
.auto { display: inline-flex; gap: 4px; align-items: center; font-size: 12px; }
textarea { width: 100%; border: none; border-radius: 0; resize: vertical; display: block; min-height: 140px; tab-size: 4; font-size: 13px; }
.kind { font-size: 11.5px; color: var(--accent-strong); }
.editor :deep(.hex) { padding: 10px; gap: 14px; }
.editor :deep(.hex .actions) { margin-top: 2px; }
.name-input { width: 100%; min-width: 0; height: 21px; padding: 0 5px; font: inherit; border-radius: 4px; }
</style>
