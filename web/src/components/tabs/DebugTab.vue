<script setup lang="ts">
// Le débogueur : le code autour de l'instruction en cours (désassemblé), les registres modifiables, les points d'arrêt, le
// pas-à-pas. Les points d'observation (qui écrit à cette adresse ?) sont dans l'onglet Mémoire.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { showInMemory } from "../../ui.js";
import { assignIfChanged, hex, hex0x, parseAddress, poll } from "../../util.js";
import Icon from "../Icon.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

interface Line {
  address: number;
  size: number;
  hex: string;
  mnemonic: string;
  operands: string;
  target?: number;
  target_symbol?: string;
  label?: string;
}
interface Frame {
  address: number;
  symbol?: string;
  return?: number;
  target_symbol?: string;
}
interface Cpu {
  r: number[];
  sp: number;
  lr: number;
  pc: number;
  xpsr: number;
}

const cpu = ref<Cpu | null>(null);
const previous = ref<Cpu | null>(null);
const lines = ref<Line[]>([]);
const breaks = ref<number[]>([]);
const frames = ref<Frame[]>([]);
/** Adresse affichée ; `null` : on suit l'instruction en cours. */
const viewAt = ref<number | null>(null);
const gotoText = ref("");
const message = ref("");
/** Point d'arrêt provisoire de « jusqu'ici » et « par-dessus » : retiré dès que l'exécution s'arrête. */
let temporary: number | null = null;
let stopPolling = () => {};

const ready = computed(() => state.loaded);
const paused = computed(() => state.loaded && !state.running);

async function refresh() {
  if (!ready.value) return;
  const c = await quiet({ cmd: "cpu" });
  if (!c.ok) return;
  const next: Cpu = { r: c.r as number[], sp: c.sp as number, lr: c.lr as number, pc: c.pc as number, xpsr: c.xpsr as number };
  if (cpu.value && JSON.stringify(cpu.value) !== JSON.stringify(next)) previous.value = cpu.value;
  assignIfChanged(cpu, next);
  assignIfChanged(breaks, ((await quiet({ cmd: "breaks" })).breakpoints as number[] | undefined) ?? []);
  if (!state.running) {
    const trace = await quiet({ cmd: "backtrace" });
    if (trace.ok) assignIfChanged(frames, trace.frames as Frame[]);
  }
  // Pendant l'exécution le code défile trop vite pour être lu : on ne le relit qu'à l'arrêt.
  if (state.running && lines.value.length > 0) return;
  const follow = viewAt.value === null;
  const d = await quiet({ cmd: "disasm", address: follow ? next.pc : (viewAt.value as number), count: 26, before: follow ? 7 : 0 });
  if (d.ok) assignIfChanged(lines, d.lines as Line[]);
}

onMounted(() => {
  stopPolling = poll(refresh, 400);
});
onBeforeUnmount(() => stopPolling());
watch(() => [state.revision, viewAt.value], refresh);
watch(
  () => state.running,
  async (running) => {
    if (running || temporary === null) return;
    await quiet({ cmd: "unbreak", address: temporary });
    temporary = null;
    viewAt.value = null;
    await refresh();
  },
);

const current = computed(() => lines.value.find((l) => l.address === cpu.value?.pc));
const stopText = computed(() => {
  const stop = state.stop;
  if (!state.loaded || state.running || !stop) return "";
  if (stop.kind === "breakpoint") return t("debug.stoppedBreak", { pc: hex0x(Number(stop.pc)) });
  if (stop.kind === "watchpoint") return t("debug.stoppedWatch", { pc: hex0x(Number(stop.pc)), address: hex0x(Number(stop.address)) });
  if (stop.kind === "fault") return t("debug.stoppedFault", { fault: String(stop.fault ?? "") });
  return "";
});

async function step() {
  message.value = "";
  const r = await quiet({ cmd: "step" });
  if (!r.ok) message.value = String(r.error);
  viewAt.value = null;
  emulator.bump();
  await refresh();
}

async function runTo(address: number) {
  if (!breaks.value.includes(address)) {
    temporary = address;
    await quiet({ cmd: "break", address });
  }
  await emulator.start();
}

/** Passe par-dessus un appel (`bl`, `blx`) : exécute jusqu'à l'instruction suivante ; sinon, un pas. */
async function stepOver() {
  const here = current.value;
  if (here && /^blx?/.test(here.mnemonic)) await runTo(here.address + here.size);
  else await step();
}

async function toggleBreak(address: number) {
  await quiet({ cmd: breaks.value.includes(address) ? "unbreak" : "break", address });
  await refresh();
}

async function goto() {
  let address = parseAddress(gotoText.value);
  if (address === undefined) {
    // Pas une adresse : un nom de fonction.
    const found = (await quiet({ cmd: "symbols", query: gotoText.value.trim() })).symbols as Array<{ name: string; address: number }> | undefined;
    const exact = found?.find((s) => s.name === gotoText.value.trim()) ?? found?.[0];
    address = exact?.address;
  }
  if (address === undefined) return void (message.value = t("debug.badAddress"));
  message.value = "";
  viewAt.value = (address & ~1) >>> 0;
}

const NAMES = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "sp", "lr", "pc", "xpsr"];
const valueOf = (name: string): number | undefined => {
  const c = cpu.value;
  if (!c) return undefined;
  if (name === "sp" || name === "lr" || name === "pc" || name === "xpsr") return c[name];
  return c.r[Number(name.slice(1))];
};
const changed = (name: string) => previous.value !== null && cpu.value !== null && valueOfIn(previous.value, name) !== valueOf(name);
function valueOfIn(c: Cpu, name: string): number | undefined {
  return name === "sp" || name === "lr" || name === "pc" || name === "xpsr" ? c[name] : c.r[Number(name.slice(1))];
}

const drafts = ref<Record<string, string>>({});
async function commit(name: string) {
  const text = drafts.value[name];
  delete drafts.value[name];
  if (text === undefined) return;
  const value = parseAddress(text);
  if (value === undefined) return void (message.value = t("debug.badValue", { register: name }));
  const r = await quiet({ cmd: "cpu_set", register: name, value });
  message.value = r.ok ? "" : String(r.error);
  if (name === "pc") viewAt.value = null;
  await refresh();
}

const flags = computed(() => {
  const x = cpu.value?.xpsr ?? 0;
  return ["N", "Z", "C", "V", "Q"].map((name, i) => ({ name, on: !!((x >>> (31 - i)) & 1) }));
});
</script>

<template>
  <div class="debug">
    <p v-if="!ready" class="muted">{{ t("debug.noApp") }}</p>
    <template v-else>
      <div class="bar">
        <button :title="state.running ? t('top.pauseHint') : t('top.resumeHint')" @click="state.running ? emulator.stop() : emulator.start()">
          <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("debug.pause") : t("debug.resume") }}
        </button>
        <button :disabled="!paused" :title="t('debug.stepHint')" @click="step"><Icon name="step" :size="14" /> {{ t("debug.step") }}</button>
        <button :disabled="!paused" :title="t('debug.overHint')" @click="stepOver">{{ t("debug.over") }}</button>
        <button :disabled="viewAt === null" :title="t('debug.followHint')" @click="viewAt = null">{{ t("debug.follow") }}</button>
        <form class="goto" @submit.prevent="goto">
          <input v-model="gotoText" class="mono" :placeholder="t('debug.goto')" :aria-label="t('debug.goto')" data-keep-keys />
        </form>
      </div>
      <p v-if="stopText" class="stop" role="status">{{ stopText }}</p>
      <p v-if="message" class="error" role="alert">{{ message }}</p>

      <div class="code mono" role="table" :aria-label="t('debug.code')">
        <template v-for="line in lines" :key="line.address">
        <div v-if="line.label" class="label">{{ line.label }}:</div>
        <div
          class="row"
          :class="{ pc: line.address === cpu?.pc, hit: breaks.includes(line.address) }"
          role="row"
        >
          <button class="dot" :title="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" :aria-label="t('debug.addBreak')" @click="toggleBreak(line.address)">
            <span :class="{ on: breaks.includes(line.address) }" />
          </button>
          <button class="addr" :title="t('debug.runTo')" :disabled="!paused" @click="runTo(line.address)">{{ hex(line.address) }}</button>
          <span class="bytes muted">{{ line.hex }}</span>
          <span class="mn">{{ line.mnemonic }}</span>
          <span class="ops">{{ line.operands }}<span v-if="line.target_symbol" class="muted"> &lt;{{ line.target_symbol }}&gt;</span></span>
          <button v-if="line.target !== undefined" class="target" :title="t('debug.followTarget')" @click="viewAt = line.target ?? null">→</button>
          <button v-if="line.target !== undefined" class="target" :title="t('debug.showInMemory')" @click="showInMemory(line.target as number)">⌗</button>
        </div>
        </template>
        <p v-if="!lines.length" class="muted pad">{{ t("debug.unreadable") }}</p>
      </div>

      <h3>{{ t("debug.stack") }}</h3>
      <p class="muted small">{{ t("debug.stackHelp") }}</p>
      <ol v-if="paused && frames.length" class="frames mono">
        <li v-for="(frame, i) in frames" :key="i">
          <button class="link" :title="t('debug.followTarget')" @click="viewAt = frame.address">{{ frame.symbol ?? hex0x(frame.address) }}</button>
          <span class="muted">{{ hex0x(frame.address) }}</span>
        </li>
      </ol>
      <p v-else class="muted small">{{ t("debug.stackRunning") }}</p>

      <h3>{{ t("debug.registers") }}</h3>
      <p class="muted small">{{ paused ? t("debug.registersHelp") : t("debug.registersRunning") }}</p>
      <div v-if="cpu" class="regs mono">
        <label v-for="name in NAMES" :key="name" :class="{ changed: changed(name) }">
          <span class="name">{{ name }}</span>
          <input
            :value="drafts[name] ?? hex(valueOf(name) ?? 0)"
            :disabled="!paused"
            :aria-label="name"
            data-keep-keys
            @focus="drafts[name] = hex(valueOf(name) ?? 0)"
            @input="(e: Event) => (drafts[name] = (e.target as HTMLInputElement).value)"
            @blur="delete drafts[name]"
            @keydown.enter.prevent="(e: KeyboardEvent) => (commit(name), (e.target as HTMLInputElement).blur())"
            @keydown.esc.prevent="(e: KeyboardEvent) => (e.target as HTMLInputElement).blur()"
          />
        </label>
        <div class="flags"><span v-for="f in flags" :key="f.name" :class="{ on: f.on }">{{ f.name }}</span></div>
      </div>

      <h3>{{ t("debug.breakpoints") }}</h3>
      <p class="muted small">{{ t("debug.breakpointsHelp") }}</p>
      <ul v-if="breaks.length" class="breaks mono">
        <li v-for="address in breaks" :key="address">
          <button class="link" @click="viewAt = address">{{ hex0x(address) }}</button>
          <button class="flat icon" :title="t('debug.removeBreak')" :aria-label="t('debug.removeBreak')" @click="toggleBreak(address)"><Icon name="trash" :size="14" /></button>
        </li>
      </ul>
      <p v-else class="muted small">{{ t("debug.noBreakpoints") }}</p>
    </template>
  </div>
</template>

<style scoped>
.debug { display: grid; gap: 8px; align-content: start; }
.bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.goto { margin-left: auto; }
.goto input { width: 9em; }
.stop { margin: 0; padding: 6px 8px; border-radius: 6px; background: var(--accent-soft); font-size: 12.5px; }
.code { border: 1px solid var(--line); border-radius: 8px; overflow: auto; max-height: 420px; font-size: 12.5px; background: var(--surface); }
.row { display: flex; align-items: center; gap: 8px; padding: 0 8px 0 2px; white-space: nowrap; }
.row:hover { background: var(--accent-soft); }
.row.pc { background: color-mix(in srgb, var(--warn) 22%, transparent); font-weight: 600; }
.dot { border: none; background: none; padding: 0 4px; width: 18px; height: 20px; display: grid; place-items: center; cursor: pointer; }
.dot span { width: 9px; height: 9px; border-radius: 50%; border: 1px solid transparent; }
.dot:hover span { border-color: var(--danger); }
.dot span.on { background: var(--danger); border-color: var(--danger); }
.addr, .target { border: none; background: none; padding: 0; font: inherit; color: var(--accent-strong); cursor: pointer; }
.addr:hover, .target:hover { text-decoration: underline; }
.addr:disabled { color: inherit; cursor: default; text-decoration: none; }
.bytes { min-width: 9ch; }
.mn { min-width: 7ch; color: var(--syn-key); }
.ops { color: var(--syn-num); flex: 1; }
.label { padding: 3px 8px 0 28px; color: var(--accent-strong); font-weight: 600; }
.frames { margin: 0; padding-left: 2.2em; display: grid; gap: 2px; }
.frames li { padding-left: 2px; }
.frames li .muted { margin-left: 8px; }
.pad { padding: 10px; margin: 0; }
h3 { margin: 6px 0 0; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.small { margin: 0; font-size: 12px; }
.regs { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 4px 10px; align-items: center; }
.regs label { display: flex; align-items: center; gap: 6px; }
.regs .name { width: 3.2ch; color: var(--muted); }
.regs input { width: 100%; min-width: 0; height: 22px; padding: 0 5px; }
.regs label.changed input { color: var(--warn); }
.flags { grid-column: 1 / -1; display: flex; gap: 8px; }
.flags span { color: var(--muted); }
.flags span.on { color: var(--text); font-weight: 700; }
.breaks { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
.breaks li { display: flex; align-items: center; gap: 8px; }
.breaks .icon { margin-left: auto; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
