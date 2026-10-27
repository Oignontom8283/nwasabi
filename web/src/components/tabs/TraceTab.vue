<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { Reply } from "@nwasabi/js";
import { hex0x } from "../../util.js";
import { poll } from "../../poll.js";
import { NAMES } from "../../svcNames.js";

const { t, te, n } = useI18n();
const emulator = useEmu();
const { quiet, call } = emulator;
const counts = ref<Array<{ svc: number; calls: number }>>([]);
const cpu = ref<Reply | null>(null);
const stdout = ref("");
const frames = ref(0);
const prevCpu = ref<Reply | null>(null);
const changed = ref(new Set<string>());
let stopPolling = () => {};


async function refresh() {
  const s = await quiet({ cmd: "svc_stats" });
  if (s.ok) {
    counts.value = [...(s.counts as Array<{ svc: number; calls: number }>)].sort((a, b) => b.calls - a.calls);
    frames.value = s.frames;
  }
  const c = await quiet({ cmd: "cpu" });
  if (c.ok) {
    const diff = new Set<string>();
    const before = prevCpu.value;
    if (before) {
      (c.r as number[]).forEach((v, i) => v !== before.r[i] && diff.add(`r${i}`));
      for (const k of ["sp", "lr", "pc", "xpsr"]) if (c[k] !== before[k]) diff.add(k);
    }
    changed.value = diff;
    prevCpu.value = c;
    cpu.value = c;
  } else cpu.value = null;
  const o = await quiet({ cmd: "stdout" });
  if (o.ok) stdout.value = o.text;
}
onMounted(() => {
  stopPolling = poll(refresh, 700);
});
onBeforeUnmount(() => stopPolling());

const flags = computed(() => {
  const x = cpu.value?.xpsr ?? 0;
  return ["N", "Z", "C", "V", "Q"].map((name, i) => ({ name, on: !!((x >>> (31 - i)) & 1) }));
});
const registers = computed(() => (cpu.value ? [...(cpu.value.r as number[]).map((v, i) => [`r${i}`, v]), ["sp", cpu.value.sp], ["lr", cpu.value.lr], ["pc", cpu.value.pc], ["xpsr", cpu.value.xpsr], ["fpscr", cpu.value.fpscr]] : []));
// Le rôle d'un registre : r0–r3 portent les arguments, r4–r11 sont conservés par les fonctions, le reste a un rôle propre.
const LABELS: Record<string, string> = { r0: "r0–r3", r4: "r4–r11" };
const ROLES = ["r0", "r4", "r12", "sp", "lr", "pc", "xpsr", "fpscr"];
const roleKey = (name: string) => (/^r[0-3]$/.test(name) ? "args" : /^r([4-9]|1[01])$/.test(name) ? "saved" : name);

async function clearOutput() {
  await call({ cmd: "stdout", clear: true });
  await refresh();
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
          <div v-for="[name, value] in registers" :key="name" class="reg" :class="{ chg: changed.has(name) }" :title="t(`trace.regInfo.${roleKey(name)}`)">
            <span class="rn">{{ name }}</span><span>{{ hex0x(value) }}</span>
          </div>
        </div>
        <p class="flags mono">
          <span v-for="f in flags" :key="f.name" class="flag" :class="{ on: f.on }" :title="t(`trace.flag.${f.name}`)">{{ f.name }}</span>
          <span class="muted"> · {{ t("trace.instructions") }} : {{ n(cpu.instructions) }}</span>
        </p>
        <dl class="notes muted">
          <template v-for="id in ROLES" :key="id"><dt class="mono">{{ LABELS[id] ?? id }}</dt><dd>{{ t(`trace.regInfo.${roleKey(id)}`) }}</dd></template>
        </dl>
        <p class="muted notes-title">{{ t("trace.flagsHelp") }}</p>
        <ul class="flagtext muted">
          <li v-for="f in flags" :key="f.name" :class="{ on: f.on }">{{ t(`trace.flag.${f.name}`) }}</li>
        </ul>
      </template>
      <p v-else class="muted">{{ t("trace.noDevice") }}</p>
    </section>

    <section class="card">
      <h2>{{ t("trace.svc") }}</h2>
      <p class="muted">{{ t("trace.svcHelp", { frames: n(frames) }) }}</p>
      <div class="gh">
        <div class="gh-head svc-row" aria-hidden="true"><span class="r">#</span><span>{{ t("trace.svcName") }}</span><span>{{ t("trace.role") }}</span><span class="r">{{ t("trace.calls") }}</span></div>
        <div class="gh-rows">
          <div v-for="c in counts" :key="c.svc" class="gh-row svc-row">
            <span class="mono muted r">{{ c.svc }}</span><span class="mono">{{ NAMES[c.svc] ?? "—" }}</span><span class="muted role">{{ te(`trace.svcInfo.${c.svc}`) ? t(`trace.svcInfo.${c.svc}`) : "" }}</span><span class="mono r">{{ n(c.calls) }}</span>
          </div>
          <p v-if="!counts.length" class="empty muted">{{ t("trace.none") }}</p>
        </div>
      </div>
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
.trace { display: grid; gap: 20px; }
.card:first-child { border-top: none; padding-top: 0; }
.card { border-top: 1px solid var(--line); padding: 16px 0 0; }
.card h2 { margin-bottom: 8px; }
.card p { margin: 0 0 10px; font-size: 12px; }
header { display: flex; gap: 6px; align-items: center; }
.spacer { flex: 1; }
.regs { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 3px 14px; margin-top: 4px; }
.reg { display: flex; justify-content: space-between; padding: 1px 4px; border-radius: 4px; }
.reg.chg { background: color-mix(in srgb, var(--warn) 20%, transparent); }
.rn { color: var(--muted); }
.flags { margin: 14px 0 16px !important; }
.flag { display: inline-block; width: 1.6em; text-align: center; border: 1px solid var(--line); border-radius: 4px; color: var(--muted); margin-right: 3px; }
.flag.on { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
.gh-row.chg { background: color-mix(in srgb, var(--warn) 20%, transparent); }
.role { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.svc-row { grid-template-columns: 34px minmax(0, 1.2fr) minmax(0, 1.6fr) 64px; }
.notes { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; margin: 10px 0 0; font-size: 12px; }
.notes dt { color: var(--text); }
.flagtext { list-style: none; margin: 8px 0 4px; padding: 0; font-size: 12px; display: grid; gap: 6px; }
.flagtext li.on { color: var(--accent-strong); font-weight: 600; }
.notes dd { margin: 0; }
.notes-title { margin-top: 18px !important; }
.r { text-align: right; }
.empty { padding: 14px; text-align: center; }
.out { background: var(--surface-2); padding: 8px; border-radius: 6px; max-height: 220px; overflow: auto; margin: 0; white-space: pre-wrap; }
</style>
