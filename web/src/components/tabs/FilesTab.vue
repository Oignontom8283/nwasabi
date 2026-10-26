<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { fromBase64, toBase64 } from "@nwasabi/js";
import { confirmDialog, promptDialog } from "../../dialogs.js";
import { download, formatBytes, hex, readBytes } from "../../util.js";
import ContextMenu from "../ContextMenu.vue";
import Icon from "../Icon.vue";

const { t, locale } = useI18n();
const emulator = inject("emulator");
const { state, call, quiet } = emulator;

const files = ref([]);
const stats = ref(null);
const corrupt = ref(null);
const selected = ref("");
const message = ref("");
const menu = ref(null);
const importInput = ref(null);
const nfsInput = ref(null);
let timer = 0;
let lastGeneration = -1;

// L'éditeur : texte d'un script, ou aperçu d'un fichier opaque.
const text = ref("");
const original = ref("");
const autoImport = ref(false);
const originalAuto = ref(false);
const preview = ref("");
const dirty = computed(() => text.value !== original.value || autoImport.value !== originalAuto.value);
const current = computed(() => files.value.find((f) => f.name === selected.value));

const paused = computed(() => !state.running);
const usedPercent = computed(() => (stats.value ? Math.round((stats.value.used / stats.value.total) * 100) : 0));

function refresh(force = false) {
  const list = quiet({ cmd: "fs_list" });
  if (!list.ok) return;
  if (!force && list.generation === lastGeneration) return;
  lastGeneration = list.generation;
  files.value = list.files;
  corrupt.value = list.corrupt_at;
  const s = quiet({ cmd: "fs_stats" });
  stats.value = s.ok ? s : null;
  if (selected.value && !list.files.some((f) => f.name === selected.value)) closeEditor();
}

function fail(reply) {
  message.value = reply.ok ? "" : reply.error;
  state.error = "";
  refresh(true);
  return reply.ok;
}

function closeEditor() {
  selected.value = "";
  text.value = original.value = preview.value = "";
}

async function select(file) {
  if (dirty.value && !(await confirmDialog(t("files.discard"), { accept: t("files.discardAccept") }))) return;
  selected.value = file.name;
  if (file.kind === "python") {
    const src = call({ cmd: "fs_read", name: file.name, python: true });
    text.value = original.value = src.text ?? "";
    autoImport.value = originalAuto.value = !!src.auto_import;
    preview.value = "";
  } else {
    text.value = original.value = "";
    const reply = call({ cmd: "fs_read", name: file.name });
    const bytes = reply.ok ? fromBase64(reply.base64).slice(0, 256) : new Uint8Array();
    preview.value = Array.from({ length: Math.ceil(bytes.length / 16) }, (_, row) => {
      const part = bytes.slice(row * 16, row * 16 + 16);
      const ascii = Array.from(part, (b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : "·")).join("");
      return `${hex(row * 16, 4)}  ${Array.from(part, (b) => hex(b, 2)).join(" ").padEnd(47)}  ${ascii}`;
    }).join("\n");
  }
}

function save() {
  if (!current.value || current.value.kind !== "python") return;
  if (fail(call({ cmd: "fs_write", name: selected.value, python: true, text: text.value, auto_import: autoImport.value }))) {
    original.value = text.value;
    originalAuto.value = autoImport.value;
  }
}

async function create() {
  const name = await promptDialog(t("files.newName"), "script.py", { title: t("files.new"), accept: t("files.create") });
  if (!name?.trim()) return;
  const python = name.trim().endsWith(".py");
  const request = python
    ? { cmd: "fs_write", name: name.trim(), python: true, text: "", auto_import: false }
    : { cmd: "fs_write", name: name.trim(), text: "" };
  if (fail(call(request))) select({ name: name.trim(), kind: python ? "python" : "other" });
}

async function remove(file) {
  if (!(await confirmDialog(t("files.confirmDelete", { name: file.name }), { danger: true, accept: t("files.delete") }))) return;
  if (fail(call({ cmd: "fs_remove", name: file.name })) && selected.value === file.name) closeEditor();
}

async function rename(file) {
  const to = await promptDialog(t("files.renameTo"), file.name, { title: t("files.rename"), accept: t("files.rename") });
  if (!to?.trim() || to === file.name) return;
  if (fail(call({ cmd: "fs_rename", from: file.name, to: to.trim() })) && selected.value === file.name) selected.value = to.trim();
}

function downloadFile(file) {
  const reply = call({ cmd: "fs_read", name: file.name });
  if (reply.ok) download(file.name, fromBase64(reply.base64));
}

async function importFiles(event) {
  for (const file of event.target.files) {
    fail(call({ cmd: "fs_write", name: file.name, base64: toBase64(await readBytes(file)) }));
  }
  event.target.value = "";
}

function exportNfs() {
  const reply = call({ cmd: "fs_export" });
  if (fail(reply)) download("files.nfs", JSON.stringify(reply.nfs, null, 1), "application/json");
}

async function importNfs(event) {
  const file = event.target.files[0];
  event.target.value = "";
  if (!file) return;
  let document;
  try {
    document = JSON.parse(await file.text());
  } catch {
    message.value = t("errors.json", { source: file.name });
    return;
  }
  const replace = await confirmDialog(t("files.importMode", { name: file.name }), { accept: t("files.replaceAll"), title: t("files.importNfs") });
  if (fail(call({ cmd: "fs_import", nfs: document, replace }))) emulator.bump();
}

function openMenu(event, file) {
  event.preventDefault();
  select(file);
  menu.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      { label: t("files.edit"), action: () => select(file), disabled: file.kind !== "python" },
      { label: t("files.rename"), action: () => rename(file) },
      { label: t("files.download"), action: () => downloadFile(file) },
      { separator: true },
      { label: t("files.delete"), action: () => remove(file), danger: true },
    ],
  };
}

function rowKey(event, file, index) {
  const go = (i) => document.getElementById(`file-${files.value[i]?.name}`)?.focus();
  if (event.key === "ArrowDown") go(Math.min(index + 1, files.value.length - 1));
  else if (event.key === "ArrowUp") go(Math.max(index - 1, 0));
  else if (event.key === "Enter") select(file);
  else if (event.key === "F2") rename(file);
  else if (event.key === "Delete") remove(file);
  else if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
    const box = event.currentTarget.getBoundingClientRect();
    openMenu({ preventDefault() {}, clientX: box.left + 30, clientY: box.bottom }, file);
  } else return;
  event.preventDefault();
}

const ICON = (f) => (f.kind === "python" ? "python" : f.kind === "system" ? "gear" : "file");

onMounted(() => {
  refresh(true);
  // L'application modifie le stockage en direct : on interroge la « génération » régulièrement.
  timer = setInterval(() => refresh(), 400);
});
onBeforeUnmount(() => clearInterval(timer));
watch(() => state.revision, () => refresh(true));
</script>

<template>
  <div class="files">
    <div class="toolbar" role="toolbar" :aria-label="t('files.toolbar')">
      <button class="flat" :title="t('files.new')" @click="create"><Icon name="plus" /> {{ t("files.new") }}</button>
      <button class="flat" :title="t('files.import')" @click="importInput.click()"><Icon name="upload" /> {{ t("files.import") }}</button>
      <span class="sep" />
      <button class="flat" :title="t('files.importNfsHint')" @click="nfsInput.click()">{{ t("files.importNfs") }}</button>
      <button class="flat" :disabled="!paused" :title="paused ? t('files.exportNfsHint') : t('common.pauseRequired')" @click="exportNfs">{{ t("files.exportNfs") }}</button>
      <input ref="importInput" type="file" multiple hidden @change="importFiles" />
      <input ref="nfsInput" type="file" accept=".nfs,application/json" hidden @change="importNfs" />
    </div>
    <p v-if="corrupt !== null" class="error">{{ t("files.corrupt", { offset: corrupt }) }}</p>
    <p v-if="message" class="error" role="alert">{{ message }}</p>

    <div class="explorer" role="listbox" :aria-label="t('files.list')">
      <div class="head" aria-hidden="true"><span /><span>{{ t("files.colName") }}</span><span>{{ t("files.colType") }}</span><span class="r">{{ t("files.colSize") }}</span></div>
      <div class="rows">
        <div
          v-for="(f, i) in files"
          :id="`file-${f.name}`"
          :key="f.name"
          class="row"
          :class="{ sel: f.name === selected }"
          role="option"
          tabindex="0"
          :aria-selected="f.name === selected"
          @click="select(f)"
          @dblclick="select(f)"
          @contextmenu="openMenu($event, f)"
          @keydown="rowKey($event, f, i)"
        >
          <span class="ico" aria-hidden="true"><Icon :name="ICON(f)" :size="14" /></span>
          <span class="name">{{ f.name }}</span>
          <span class="type muted">{{ t(`files.kinds.${f.kind.split(":")[0]}`) }}</span>
          <span class="size mono muted r">{{ formatBytes(f.size, locale) }}</span>
        </div>
        <p v-if="!files.length" class="empty muted">{{ t("files.empty") }}</p>
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
        <span v-if="dirty" class="muted">● {{ t("files.modified") }}</span>
        <span class="spacer" />
        <label v-if="current.kind === 'python'" class="auto"><input v-model="autoImport" type="checkbox" /> {{ t("files.autoImport") }}</label>
        <button v-if="current.kind === 'python'" class="primary" :disabled="!dirty" title="Ctrl+S" @click="save">{{ t("files.save") }}</button>
        <button :aria-label="t('common.close')" class="flat icon" @click="closeEditor"><Icon name="close" :size="14" /></button>
      </header>
      <textarea
        v-if="current.kind === 'python'"
        v-model="text"
        class="mono"
        rows="12"
        spellcheck="false"
        data-keep-keys
        :aria-label="selected"
        @keydown.ctrl.s.prevent="save"
        @keydown.meta.s.prevent="save"
      />
      <template v-else>
        <p class="muted pad">{{ t("files.opaque") }}</p>
        <pre class="mono preview">{{ preview }}</pre>
      </template>
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
.rows { max-height: 260px; overflow: auto; }
.row { height: 25px; cursor: default; outline-offset: -2px; }
.row:hover { background: var(--accent-soft); }
.row.sel { background: color-mix(in srgb, var(--accent) 24%, transparent); }
.row .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row .type, .row .size { font-size: 12px; }
.r { text-align: right; }
.ico { display: inline-flex; color: var(--muted); }
.empty { margin: 0; padding: 18px; text-align: center; }
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
.pad { margin: 6px 10px; font-size: 12px; }
.preview { margin: 0; padding: 6px 10px; overflow: auto; background: var(--surface-2); }
</style>
