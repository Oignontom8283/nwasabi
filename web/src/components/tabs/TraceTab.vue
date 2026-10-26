<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { hex0x } from "../../util.js";

const { t, n } = useI18n();
const emulator = inject("emulator");
const { quiet, call } = emulator;
const counts = ref([]);
const cpu = ref(null);
const stdout = ref("");
const frames = ref(0);
const prevCpu = ref(null);
const changed = ref(new Set());
let timer = 0;

// Noms des appels système du noyau d'Epsilon (et des appels privés de nwasabi).
const NAMES = {
  1: "backlight_brightness", 2: "backlight_set_brightness", 3: "battery_is_charging", 4: "battery_level",
  5: "battery_voltage", 18: "display_pull_rect", 19: "display_push_rect", 20: "display_push_rect_uniform",
  21: "display_wait_for_vblank", 22: "events_copy_text", 23: "events_get_event", 24: "events_is_defined", 29: "fcc_id",
  33: "keyboard_pop_state", 34: "keyboard_scan", 39: "pcb_version", 45: "random", 47: "serial_number_copy",
  48: "timing_millis", 49: "timing_msleep", 50: "timing_usleep", 52: "usb_is_plugged", 55: "events_repeat_count",
  56: "compilation_flags", 128: "draw_string (nwasabi)", 129: "_sbrk (nwasabi)", 130: "_write (nwasabi)",
  137: "exit (nwasabi)",
};

function refresh() {
  const s = quiet({ cmd: "svc_stats" });
  if (s.ok) {
    counts.value = [...s.counts].sort((a, b) => b.calls - a.calls);
    frames.value = s.frames;
  }
  const c = quiet({ cmd: "cpu" });
  if (c.ok) {
    const diff = new Set();
    if (prevCpu.value) {
      c.r.forEach((v, i) => v !== prevCpu.value.r[i] && diff.add(`r${i}`));
      for (const k of ["sp", "lr", "pc", "xpsr"]) if (c[k] !== prevCpu.value[k]) diff.add(k);
    }
    changed.value = diff;
    prevCpu.value = c;
    cpu.value = c;
  } else cpu.value = null;
  const o = quiet({ cmd: "stdout" });
  if (o.ok) stdout.value = o.text;
}
onMounted(() => {
  refresh();
  timer = setInterval(refresh, 700);
});
onBeforeUnmount(() => clearInterval(timer));

const flags = computed(() => {
  const x = cpu.value?.xpsr ?? 0;
  return ["N", "Z", "C", "V", "Q"].map((name, i) => ({ name, on: !!((x >>> (31 - i)) & 1) }));
});
const registers = computed(() => (cpu.value ? [...cpu.value.r.map((v, i) => [`r${i}`, v]), ["sp", cpu.value.sp], ["lr", cpu.value.lr], ["pc", cpu.value.pc], ["xpsr", cpu.value.xpsr], ["fpscr", cpu.value.fpscr]] : []));

function clearOutput() {
  call({ cmd: "stdout", clear: true });
  refresh();
}
async function copyOutput() {
  try {
    await navigator.clipboard.writeText(stdout.value);
  } catch {
    /* refusé */
  }
}
</script>

<template>
  <div class="trace">
    <section class="card">
      <h2>{{ t("trace.cpu") }}</h2>
      <p class="muted">{{ t("trace.cpuHelp") }}</p>
      <template v-if="cpu">
        <div class="regs mono">
          <div v-for="[name, value] in registers" :key="name" class="reg" :class="{ chg: changed.has(name) }">
            <span class="rn">{{ name }}</span><span>{{ hex0x(value) }}</span>
          </div>
        </div>
        <p class="flags mono">
          <span v-for="f in flags" :key="f.name" class="flag" :class="{ on: f.on }" :title="t(`trace.flag.${f.name}`)">{{ f.name }}</span>
          <span class="muted"> · {{ t("trace.instructions") }} : {{ n(cpu.instructions) }}</span>
        </p>
      </template>
      <p v-else class="muted">{{ t("trace.noDevice") }}</p>
    </section>

    <section class="card">
      <h2>{{ t("trace.svc") }}</h2>
      <p class="muted">{{ t("trace.svcHelp", { frames: n(frames) }) }}</p>
      <table class="mono">
        <thead><tr><th>#</th><th>{{ t("trace.svcName") }}</th><th class="num">{{ t("trace.calls") }}</th></tr></thead>
        <tbody>
          <tr v-for="c in counts" :key="c.svc"><td>{{ c.svc }}</td><td>{{ NAMES[c.svc] ?? "—" }}</td><td class="num">{{ n(c.calls) }}</td></tr>
          <tr v-if="!counts.length"><td colspan="3" class="muted">{{ t("trace.none") }}</td></tr>
        </tbody>
      </table>
    </section>

    <section class="card">
      <header>
        <h2>{{ t("trace.stdout") }}</h2>
        <span class="spacer" />
        <button @click="copyOutput">{{ t("trace.copy") }}</button>
        <button @click="clearOutput">{{ t("trace.clear") }}</button>
      </header>
      <pre class="mono out" tabindex="0" data-keep-keys>{{ stdout || t("trace.empty") }}</pre>
    </section>
  </div>
</template>

<style scoped>
.trace { display: grid; gap: 12px; }
.card:first-child { border-top: none; padding-top: 0; }
.card { border-top: 1px solid var(--line); padding: 10px 0 0; }
.card h2 { margin-bottom: 4px; }
.card p { margin: 0 0 6px; font-size: 12px; }
header { display: flex; gap: 6px; align-items: center; }
.spacer { flex: 1; }
.regs { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 2px 14px; }
.reg { display: flex; justify-content: space-between; padding: 1px 4px; border-radius: 4px; }
.reg.chg { background: color-mix(in srgb, var(--warn) 20%, transparent); }
.rn { color: var(--muted); }
.flags { margin: 8px 0 0; }
.flag { display: inline-block; width: 1.6em; text-align: center; border: 1px solid var(--line); border-radius: 4px; color: var(--muted); margin-right: 3px; }
.flag.on { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
.num { text-align: right; }
.out { background: var(--surface-2); padding: 8px; border-radius: 6px; max-height: 220px; overflow: auto; margin: 0; white-space: pre-wrap; }
</style>
