<script setup>
import { inject, onBeforeUnmount, onMounted, ref, watch } from "vue";

const { state, call } = inject("emulator");
const info = ref(null);
let timer = 0;

const hex = (n) => "0x" + (n >>> 0).toString(16).padStart(8, "0");
const ADDRESS_KEYS = /address|base|start|end|header|entry|stack|draw_string|storage_address|flash|ram/;

function fmt(key, value) {
  if (typeof value === "number" && ADDRESS_KEYS.test(key)) return hex(value);
  return String(value);
}

function refresh() {
  const r = call({ cmd: "inspect" });
  if (r.ok) info.value = r;
}
onMounted(() => { refresh(); timer = setInterval(refresh, 1000); });
onBeforeUnmount(() => clearInterval(timer));
watch(() => state.revision, refresh);

const SECTIONS = [
  ["slot_info", "SlotInfo"],
  ["slot_header", "En-tête du slot"],
  ["kernel_header", "KernelHeader"],
  ["userland_header", "UserlandHeader"],
  ["vector_table", "Table des vecteurs du userland"],
  ["storage", "Système de fichiers"],
];
</script>

<template>
  <div v-if="info">
    <p class="muted">Relu depuis la mémoire à chaque fois : si l'application les écrase, on le voit ici.</p>
    <template v-for="[key, title] in SECTIONS" :key="key">
      <h2>{{ title }} <span v-if="info.structures[key] && info.structures[key].valid === false" class="error">invalide</span></h2>
      <table v-if="info.structures[key]" class="mono">
        <tr v-for="(value, name) in info.structures[key]" :key="name">
          <template v-if="typeof value !== 'object'">
            <th>{{ name }}</th><td>{{ fmt(name, value) }}</td>
          </template>
          <template v-else>
            <th>{{ name }}</th><td>{{ hex(value.start) }} – {{ hex(value.end) }}</td>
          </template>
        </tr>
      </table>
      <p v-else class="muted">introuvable</p>
    </template>
    <h2>Nom de l'appareil</h2>
    <p class="mono">{{ info.structures.device_name === "" ? "(aucun)" : info.structures.device_name ?? "—" }}</p>
    <h2>Applications installées</h2>
    <table class="mono">
      <tr v-for="a in info.structures.apps" :key="a.address">
        <td>{{ a.name }}</td><td>{{ hex(a.address) }}</td><td>API {{ a.api_level }}</td><td>{{ a.size }} octets</td>
      </tr>
      <tr v-if="!info.structures.apps.length"><td class="muted">aucune</td></tr>
    </table>
    <template v-if="info.app">
      <h2>Application liée</h2>
      <table class="mono">
        <tr v-for="(value, name) in info.app" :key="name">
          <th>{{ name }}</th>
          <td v-if="Array.isArray(value)">{{ value.join("; ") || "—" }}</td>
          <td v-else-if="typeof value === 'object'">{{ hex(value.start) }} – {{ hex(value.end) }}</td>
          <td v-else>{{ fmt(name, value) }}</td>
        </tr>
      </table>
    </template>
  </div>
</template>
