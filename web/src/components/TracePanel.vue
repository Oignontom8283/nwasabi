<script setup>
import { inject, onBeforeUnmount, onMounted, ref } from "vue";

const { state, call } = inject("emulator");
const counts = ref([]);
const cpu = ref(null);
const stdout = ref("");
let timer = 0;

// Noms des appels système du noyau d'Epsilon (et des appels privés de nwasabi).
const NAMES = {
  1: "backlight_brightness", 2: "backlight_set_brightness", 3: "battery_is_charging", 4: "battery_level",
  5: "battery_voltage", 18: "display_pull_rect", 19: "display_push_rect", 20: "display_push_rect_uniform",
  21: "display_wait_for_vblank", 23: "events_get_event", 29: "fcc_id", 33: "keyboard_pop_state",
  34: "keyboard_scan", 39: "pcb_version", 45: "random", 47: "serial_number_copy", 48: "timing_millis",
  49: "timing_msleep", 50: "timing_usleep", 52: "usb_is_plugged", 56: "compilation_flags",
  128: "draw_string (privé)", 129: "_sbrk (privé)", 130: "_write (privé)", 137: "fin de l'application (privé)",
};
const hex = (n) => "0x" + (n >>> 0).toString(16).padStart(8, "0");

function refresh() {
  const s = call({ cmd: "svc_stats" });
  if (s.ok) counts.value = s.counts;
  const c = call({ cmd: "cpu" });
  cpu.value = c.ok ? c : null;
  if (!c.ok) state.error = "";
  const o = call({ cmd: "stdout" });
  if (o.ok) stdout.value = o.text;
}
onMounted(() => { refresh(); timer = setInterval(refresh, 800); });
onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <div>
    <h2>Appels système</h2>
    <table class="mono">
<tbody>
      <tr v-for="c in counts" :key="c.svc"><td>#{{ c.svc }}</td><td>{{ NAMES[c.svc] ?? "—" }}</td><td>{{ c.calls }}</td></tr>
      <tr v-if="!counts.length"><td class="muted">aucun</td></tr>
    </tbody>
</table>
    <h2>Processeur</h2>
    <table v-if="cpu" class="mono">
<tbody>
      <tr v-for="(v, i) in cpu.r" :key="i"><th>r{{ i }}</th><td>{{ hex(v) }}</td></tr>
      <tr><th>sp</th><td>{{ hex(cpu.sp) }}</td></tr>
      <tr><th>lr</th><td>{{ hex(cpu.lr) }}</td></tr>
      <tr><th>pc</th><td>{{ hex(cpu.pc) }}</td></tr>
      <tr><th>xpsr</th><td>{{ hex(cpu.xpsr) }}</td></tr>
      <tr><th>instructions</th><td>{{ cpu.instructions }}</td></tr>
    </tbody>
</table>
    <h2>Sortie standard</h2>
    <pre class="mono out">{{ stdout || "(vide)" }}</pre>
  </div>
</template>

<style scoped>
.out { background: var(--bg); padding: 8px; border-radius: 6px; max-height: 200px; overflow: auto; }
</style>
