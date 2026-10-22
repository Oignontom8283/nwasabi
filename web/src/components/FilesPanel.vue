<script setup>
import { inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { fromBase64, toBase64 } from "@nwasabi/js";

const { state, call } = inject("emulator");
const files = ref([]);
const stats = ref(null);
const corrupt = ref(null);
const generation = ref(0);
const selected = ref("");
const text = ref("");
const autoImport = ref(false);
const newName = ref("script.py");
const message = ref("");
let timer = 0;
let lastGeneration = -1;

function refresh(force = false) {
  const list = call({ cmd: "fs_list" });
  if (!list.ok) return;
  if (!force && list.generation === lastGeneration) return;
  lastGeneration = list.generation;
  generation.value = list.generation;
  files.value = list.files;
  corrupt.value = list.corrupt_at;
  const s = call({ cmd: "fs_stats" });
  stats.value = s.ok ? s : null;
}

function show(result) {
  message.value = result.ok ? "" : result.error;
  refresh(true);
  return result.ok;
}

function select(file) {
  selected.value = file.name;
  if (file.kind === "python") {
    const src = call({ cmd: "fs_read", name: file.name, python: true });
    text.value = src.text ?? "";
    autoImport.value = !!src.auto_import;
  } else {
    text.value = "";
  }
}

function save() {
  show(call({ cmd: "fs_write", name: selected.value, python: true, text: text.value, auto_import: autoImport.value }));
}

function create() {
  const name = newName.value.trim();
  if (!name) return;
  const python = name.endsWith(".py");
  const request = python
    ? { cmd: "fs_write", name, python: true, text: "", auto_import: false }
    : { cmd: "fs_write", name, text: "" };
  if (show(call(request))) selected.value = name;
}

function remove(name) {
  if (show(call({ cmd: "fs_remove", name })) && selected.value === name) selected.value = "";
}

function download(name) {
  const reply = call({ cmd: "fs_read", name });
  if (!reply.ok) return;
  const url = URL.createObjectURL(new Blob([fromBase64(reply.base64)]));
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  URL.revokeObjectURL(url);
}

async function upload(event) {
  for (const file of event.target.files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    show(call({ cmd: "fs_write", name: file.name, base64: toBase64(bytes) }));
  }
  event.target.value = "";
}

onMounted(() => {
  refresh(true);
  // L'application modifie le stockage en direct : on interroge la « génération » régulièrement.
  timer = setInterval(() => refresh(), 400);
});
onBeforeUnmount(() => clearInterval(timer));
watch(() => state.revision, () => refresh(true));
</script>

<template>
  <div>
    <h2>Fichiers de la calculatrice</h2>
    <p v-if="stats" class="muted mono">
      {{ stats.used }} / {{ stats.total }} octets utilisés · {{ stats.records }} fichier(s) · génération {{ generation }}
    </p>
    <p v-if="corrupt !== null" class="error">Système de fichiers abîmé à l'octet {{ corrupt }} (par l'application ?).</p>
    <p v-if="message" class="error">{{ message }}</p>
    <table>
      <thead><tr><th>Nom</th><th>Type</th><th>Taille</th><th /></tr></thead>
      <tbody>
        <tr v-for="f in files" :key="f.name" :class="{ sel: f.name === selected }">
          <td><a href="#" @click.prevent="select(f)">{{ f.name }}</a></td>
          <td>{{ f.kind }}</td>
          <td class="mono">{{ f.size }}</td>
          <td class="actions">
            <button @click="download(f.name)">↓</button>
            <button @click="remove(f.name)">✕</button>
          </td>
        </tr>
        <tr v-if="!files.length"><td colspan="4" class="muted">aucun fichier</td></tr>
      </tbody>
    </table>
    <div class="row">
      <input v-model="newName" placeholder="nom.py" />
      <button @click="create">Créer</button>
      <label class="button">Importer…<input type="file" multiple hidden @change="upload" /></label>
    </div>
    <div v-if="selected" class="editor">
      <h2>{{ selected }}</h2>
      <textarea v-if="files.find((f) => f.name === selected)?.kind === 'python'" v-model="text" rows="10" spellcheck="false" class="mono" />
      <p v-else class="muted">Format opaque : seul le téléchargement est proposé.</p>
      <label v-if="files.find((f) => f.name === selected)?.kind === 'python'">
        <input v-model="autoImport" type="checkbox" /> importé automatiquement
      </label>
      <button v-if="files.find((f) => f.name === selected)?.kind === 'python'" class="primary" @click="save">Enregistrer</button>
    </div>
  </div>
</template>

<style scoped>
.row { display: flex; gap: 6px; margin: 10px 0; align-items: center; }
.button { border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; cursor: pointer; background: var(--key); }
.actions { text-align: right; white-space: nowrap; }
.sel td { background: color-mix(in srgb, var(--accent) 12%, transparent); }
textarea { width: 100%; display: block; margin-bottom: 6px; }
a { color: var(--accent); }
</style>
