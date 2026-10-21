<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";

const { state, call } = inject("emulator");
const regions = ref([]);
const usage = ref(null);
const address = ref("0x20000000");
const rows = ref([]);
const changes = ref([]);
const error = ref("");
let cursor = 0;
let timer = 0;

const LINES = 16;
const hex = (n, w = 8) => n.toString(16).padStart(w, "0");

function readDump() {
  const start = parseInt(address.value, address.value.startsWith("0x") ? 16 : 10);
  if (Number.isNaN(start)) { error.value = "adresse invalide"; return; }
  const reply = call({ cmd: "memory_read", address: start >>> 0, length: LINES * 16 });
  error.value = "";
  if (!reply.ok) { rows.value = []; error.value = reply.error; state.error = ""; return; }
  const bytes = reply.hex.match(/../g).map((b) => parseInt(b, 16));
  rows.value = Array.from({ length: LINES }, (_, i) => {
    const part = bytes.slice(i * 16, i * 16 + 16);
    return {
      address: (start + i * 16) >>> 0,
      hex: part.map((b) => hex(b, 2)).join(" "),
      text: part.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : "·")).join(""),
    };
  });
}

function refresh() {
  const r = call({ cmd: "regions" });
  if (r.ok) regions.value = r.regions;
  const u = call({ cmd: "usage" });
  usage.value = u.ok ? u : null;
  if (!u.ok) state.error = "";
  const c = call({ cmd: "changes", cursor });
  if (c.ok) {
    changes.value = c.ranges.slice(0, 12);
    cursor = c.cursor;
  }
  readDump();
}

const heapPercent = computed(() => {
  const u = usage.value;
  if (!u?.heap_high_water) return 0;
  return Math.round(((u.heap_high_water - u.heap.start) / (u.heap.end - u.heap.start)) * 100);
});
const stackPercent = computed(() => {
  const u = usage.value;
  if (u?.stack_low_water == null) return 0;
  return Math.round(((u.stack.end - u.stack_low_water) / (u.stack.end - u.stack.start)) * 100);
});

onMounted(() => { refresh(); timer = setInterval(refresh, 700); });
onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <div>
    <h2>Zones mémoire</h2>
    <table class="mono">
<tbody>
      <tr v-for="r in regions" :key="r.name">
        <td>{{ r.name }}</td><td>0x{{ hex(r.base) }}</td><td>{{ (r.size / 1024).toFixed(0) }} Kio</td><td>{{ r.perms }}</td>
        <td><a href="#" @click.prevent="address = '0x' + hex(r.base); readDump()">voir</a></td>
      </tr>
    </tbody>
</table>

    <template v-if="usage">
      <h2>Utilisation par l'application</h2>
      <p class="mono">
        tas : {{ heapPercent }} % (limite 0x{{ hex(usage.heap_break) }}) ·
        pile : {{ stackPercent }} % (sp 0x{{ hex(usage.sp) }})
      </p>
      <div class="bar"><div :style="{ width: heapPercent + '%' }" /></div>
      <div class="bar"><div :style="{ width: stackPercent + '%' }" /></div>
    </template>

    <h2>Octets</h2>
    <div class="row">
      <input v-model="address" class="mono" @keyup.enter="readDump" />
      <button @click="readDump">Lire</button>
    </div>
    <p v-if="error" class="error">{{ error }}</p>
    <pre v-else class="dump">
<template v-for="r in rows" :key="r.address">{{ hex(r.address) }}  {{ r.hex }}  {{ r.text }}
</template></pre>

    <h2>Modifié récemment</h2>
    <p class="mono muted">
      <span v-for="c in changes" :key="c.start">0x{{ hex(c.start) }}+{{ c.len }}&nbsp; </span>
      <span v-if="!changes.length">rien depuis le dernier rafraîchissement</span>
    </p>
  </div>
</template>

<style scoped>
.row { display: flex; gap: 6px; margin-bottom: 8px; }
.dump { background: var(--bg); padding: 8px; border-radius: 6px; overflow-x: auto; margin: 0; }
.bar { height: 8px; background: var(--bg); border-radius: 4px; margin-bottom: 6px; overflow: hidden; }
.bar div { height: 100%; background: var(--accent); }
a { color: var(--accent); }
</style>
