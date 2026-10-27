<script setup lang="ts">
// Le code de l'application : désassemblé autour de l'instruction en cours, avec les commandes d'exécution (reprendre, pas,
// par-dessus, sortir), les points d'arrêt, la pile d'appels, les registres (modifiables en pause) et les registres de faute.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { showInMemory, ui } from "../../ui.js";
import { assignIfChanged, hex, hex0x, parseAddress, poll } from "../../util.js";
import FaultRegisters from "../FaultRegisters.vue";
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
/** Où l'on est : le nom de la fonction et le décalage. */
const here = ref("");
/** Adresse affichée ; `null` : on suit l'instruction en cours. */
const viewAt = ref<number | null>(null);
const gotoText = ref("");
const message = ref("");
/** Point d'arrêt provisoire de « jusqu'ici », « par-dessus » et « sortir » : retiré dès que l'exécution s'arrête. */
let temporary: number | null = null;
let stopPolling = () => {};

const paused = computed(() => state.loaded && !state.running);

async function refresh() {
  if (!state.loaded) return;
  const c = await quiet({ cmd: "cpu" });
  if (!c.ok) return;
  const next: Cpu = { r: c.r as number[], sp: c.sp as number, lr: c.lr as number, pc: c.pc as number, xpsr: c.xpsr as number };
  if (cpu.value && JSON.stringify(cpu.value) !== JSON.stringify(next)) previous.value = cpu.value;
  assignIfChanged(cpu, next);
  assignIfChanged(breaks, ((await quiet({ cmd: "breaks" })).breakpoints as number[] | undefined) ?? []);
  // Pendant l'exécution le code défile trop vite pour être lu : on ne le relit qu'à l'arrêt.
  if (state.running && lines.value.length > 0) return;
  const trace = await quiet({ cmd: "backtrace" });
  if (trace.ok) {
    const list = trace.frames as Frame[];
    assignIfChanged(frames, list);
    here.value = list[0]?.symbol ?? "";
  }
  const follow = viewAt.value === null;
  const d = await quiet({ cmd: "disasm", address: follow ? next.pc : (viewAt.value as number), count: 28, before: follow ? 8 : 0 });
  if (d.ok) assignIfChanged(lines, d.lines as Line[]);
}

onMounted(() => {
  stopPolling = poll(refresh, 400);
  addEventListener("keydown", onKey);
});
onBeforeUnmount(() => {
  stopPolling();
  removeEventListener("keydown", onKey);
});
watch(() => [state.revision, viewAt.value], refresh);
watch(
  () => ui.debugGoto,
  (goto) => goto && (viewAt.value = (goto.address & ~1) >>> 0),
  { immediate: true },
);
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
const status = computed((): { kind: string; text: string } => {
  if (!state.loaded) return { kind: "idle", text: t("debug.status.idle") };
  if (state.running) return { kind: "running", text: t("debug.status.running") };
  const stop = state.stop;
  if (stop?.kind === "breakpoint") return { kind: "paused", text: t("debug.status.breakpoint", { pc: hex0x(Number(stop.pc)) }) };
  if (stop?.kind === "watchpoint") return { kind: "paused", text: t("debug.status.watchpoint", { pc: hex0x(Number(stop.pc)), address: hex0x(Number(stop.address)) }) };
  if (stop?.kind === "fault") return { kind: "fault", text: t("debug.status.fault") };
  if (stop?.kind === "exited") return { kind: "idle", text: t("debug.status.exited", { code: Number(stop.code) }) };
  return { kind: "paused", text: t("debug.status.paused") };
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
  const instruction = current.value;
  if (instruction && /^blx?$/.test(instruction.mnemonic)) await runTo(instruction.address + instruction.size);
  else await step();
}

/** Sort de la fonction : exécute jusqu'à l'adresse de retour (lr). */
async function stepOut() {
  const lr = cpu.value?.lr;
  if (lr) await runTo((lr & ~1) >>> 0);
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
    address = (found?.find((s) => s.name === gotoText.value.trim()) ?? found?.[0])?.address;
  }
  if (address === undefined) return void (message.value = t("debug.badAddress"));
  message.value = "";
  viewAt.value = (address & ~1) >>> 0;
}

// Raccourcis des débogueurs usuels : F5 reprendre ou pause, F10 par-dessus, F11 pas, Maj + F11 sortir.
function onKey(event: KeyboardEvent) {
  if (ui.tab !== "debug" || ui.debugView !== "code" || !state.loaded) return;
  if (event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable]")) return;
  if (event.key === "F5") void (state.running ? emulator.stop() : emulator.start());
  else if (event.key === "F10" && paused.value) void stepOver();
  else if (event.key === "F11" && paused.value) void (event.shiftKey ? stepOut() : step());
  else return;
  event.preventDefault();
}

const NAMES = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "sp", "lr", "pc"];
const valueIn = (c: Cpu, name: string): number => (name === "sp" || name === "lr" || name === "pc" ? c[name] : c.r[Number(name.slice(1))] ?? 0);
const changed = (name: string) => previous.value !== null && cpu.value !== null && valueIn(previous.value, name) !== valueIn(cpu.value, name);

const drafts = ref<Record<string, string>>({});
async function commit(name: string) {
  const text = drafts.value[name];
  delete drafts.value[name];
  if (text === undefined) return;
  const value = parseAddress(text.trim().replace(/^(?![0-9]|0x)/i, "0x"));
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
  <div class="code-view">
    <p v-if="!state.loaded" class="empty muted">{{ t("debug.noApp") }}</p>
    <template v-else>
      <div class="status" :class="status.kind" role="status">
        <span class="dot" aria-hidden="true" />
        <span class="text">{{ status.text }}</span>
        <span v-if="here" class="mono here" :title="t('debug.hereHint')">{{ here }}</span>
      </div>

      <div class="bar" role="toolbar" :aria-label="t('debug.controls')">
        <button class="primary" :title="state.running ? t('debug.pauseHint') : t('debug.resumeHint')" @click="state.running ? emulator.stop() : emulator.start()">
          <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("debug.pause") : t("debug.resume") }} <kbd>F5</kbd>
        </button>
        <button :disabled="!paused" :title="t('debug.stepHint')" @click="step"><Icon name="step" :size="14" /> {{ t("debug.step") }} <kbd>F11</kbd></button>
        <button :disabled="!paused" :title="t('debug.overHint')" @click="stepOver">{{ t("debug.over") }} <kbd>F10</kbd></button>
        <button :disabled="!paused" :title="t('debug.outHint')" @click="stepOut">{{ t("debug.out") }} <kbd>Maj F11</kbd></button>
        <form class="goto" @submit.prevent="goto">
          <input v-model="gotoText" class="mono" :placeholder="t('debug.goto')" :aria-label="t('debug.goto')" :title="t('debug.gotoHint')" data-keep-keys />
        </form>
      </div>
      <p v-if="message" class="error" role="alert">{{ message }}</p>

      <div class="code mono" role="table" :aria-label="t('debug.code')">
        <div class="head muted" role="row" aria-hidden="true"><span class="c-dot" /><span class="c-addr">{{ t("debug.colAddress") }}</span><span class="c-bytes">{{ t("debug.colBytes") }}</span><span>{{ t("debug.colInstruction") }}</span></div>
        <template v-for="line in lines" :key="line.address">
          <div v-if="line.label" class="label">{{ line.label }}:</div>
          <div class="row" :class="{ pc: line.address === cpu?.pc, hit: breaks.includes(line.address) }" role="row">
            <button class="dot" :title="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" :aria-label="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" @click="toggleBreak(line.address)">
              <span :class="{ on: breaks.includes(line.address) }" />
            </button>
            <span class="arrow" aria-hidden="true">{{ line.address === cpu?.pc ? "▶" : "" }}</span>
            <button class="addr" :title="t('debug.runTo')" :disabled="!paused" @click="runTo(line.address)">{{ hex(line.address) }}</button>
            <span class="bytes muted">{{ line.hex }}</span>
            <span class="mn">{{ line.mnemonic }}</span>
            <span class="ops">{{ line.operands }}<span v-if="line.target_symbol" class="muted"> &lt;{{ line.target_symbol }}&gt;</span></span>
            <button v-if="line.target !== undefined" class="target" :title="t('debug.followTarget')" @click="viewAt = line.target ?? null">→</button>
            <button v-if="line.target !== undefined" class="target" :title="t('debug.showInMemory')" @click="showInMemory(line.target as number)"><Icon name="chip" :size="12" /></button>
          </div>
        </template>
        <p v-if="!lines.length" class="muted pad">{{ t("debug.unreadable") }}</p>
      </div>
      <p class="legend muted">
        <button v-if="viewAt !== null" class="link" @click="viewAt = null">{{ t("debug.follow") }}</button>
        <span>{{ t("debug.legend") }}</span>
      </p>

      <section>
        <h3>{{ t("debug.stack") }}</h3>
        <p class="muted small">{{ t("debug.stackHelp") }}</p>
        <ol v-if="paused && frames.length" class="frames mono">
          <li v-for="(frame, i) in frames" :key="i">
            <button class="link" :title="t('debug.followTarget')" @click="viewAt = frame.address">{{ frame.symbol ?? hex0x(frame.address) }}</button>
            <span class="muted">{{ hex0x(frame.address) }}</span>
          </li>
        </ol>
        <p v-else class="muted small">{{ t("debug.stackRunning") }}</p>
      </section>

      <section>
        <h3>{{ t("debug.registers") }}</h3>
        <p class="muted small">{{ paused ? t("debug.registersHelp") : t("debug.registersRunning") }}</p>
        <div v-if="cpu" class="regs mono">
          <label v-for="name in NAMES" :key="name" :class="{ changed: changed(name) }">
            <span class="name">{{ name }}</span>
            <input
              :value="drafts[name] ?? hex(valueIn(cpu, name))"
              :disabled="!paused"
              :aria-label="name"
              data-keep-keys
              @focus="drafts[name] = hex(valueIn(cpu, name))"
              @input="(e: Event) => (drafts[name] = (e.target as HTMLInputElement).value)"
              @blur="delete drafts[name]"
              @keydown.enter.prevent="(e: KeyboardEvent) => (commit(name), (e.target as HTMLInputElement).blur())"
              @keydown.esc.prevent="(e: KeyboardEvent) => (e.target as HTMLInputElement).blur()"
            />
          </label>
          <div class="flags" :title="t('debug.flagsHint')"><span class="muted">{{ t("debug.flags") }}</span><span v-for="f in flags" :key="f.name" :class="{ on: f.on }">{{ f.name }}</span></div>
        </div>
      </section>

      <section>
        <h3>{{ t("debug.breakpoints") }}</h3>
        <p class="muted small">{{ t("debug.breakpointsHelp") }}</p>
        <ul v-if="breaks.length" class="breaks mono">
          <li v-for="address in breaks" :key="address">
            <button class="link" @click="viewAt = address">{{ hex0x(address) }}</button>
            <button class="flat icon" :title="t('debug.removeBreak')" :aria-label="t('debug.removeBreak')" @click="toggleBreak(address)"><Icon name="trash" :size="14" /></button>
          </li>
        </ul>
        <p v-else class="muted small">{{ t("debug.noBreakpoints") }}</p>
      </section>

      <section>
        <h3>{{ t("debug.faultRegisters") }}</h3>
        <p class="muted small">{{ t("debug.faultRegistersHelp") }}</p>
        <FaultRegisters />
      </section>
    </template>
  </div>
</template>

<style scoped>
.code-view { display: grid; gap: 10px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
.status { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); font-size: 13px; }
.status .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--muted); flex: none; }
.status.running .dot { background: var(--ok); }
.status.paused .dot { background: var(--warn); }
.status.fault .dot { background: var(--danger); }
.status.fault .text { color: var(--danger); }
.status .here { margin-left: auto; color: var(--accent-strong); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
kbd { font: 10.5px ui-monospace, monospace; color: var(--muted); border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; margin-left: 2px; background: transparent; }
button.primary kbd { color: inherit; border-color: currentColor; opacity: 0.75; }
.goto { margin-left: auto; }
.goto input { width: 11em; }
.code { border: 1px solid var(--line); border-radius: 8px; overflow: auto; max-height: 440px; font-size: 12.5px; background: var(--surface); }
.head { display: flex; gap: 8px; padding: 3px 8px 3px 2px; border-bottom: 1px solid var(--line); position: sticky; top: 0; background: var(--surface-2); font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; }
.c-dot { width: 18px; }
.c-addr { width: 8ch; margin-left: 14px; }
.c-bytes { min-width: 9ch; }
.row { display: flex; align-items: center; gap: 8px; padding: 0 8px 0 2px; white-space: nowrap; }
.row:hover { background: var(--accent-soft); }
.row.pc { background: color-mix(in srgb, var(--warn) 22%, transparent); font-weight: 600; }
.arrow { width: 6px; font-size: 9px; color: var(--warn); }
.dot { border: none; background: none; padding: 0 4px; width: 18px; height: 20px; display: grid; place-items: center; cursor: pointer; }
.dot span { width: 9px; height: 9px; border-radius: 50%; border: 1px solid transparent; }
.dot:hover span { border-color: var(--danger); }
.dot span.on { background: var(--danger); border-color: var(--danger); }
.target { display: inline-grid; place-items: center; }
.addr, .target { border: none; background: none; padding: 0; font: inherit; color: var(--accent-strong); cursor: pointer; }
.addr:hover, .target:hover { text-decoration: underline; }
.addr:disabled { color: inherit; cursor: default; text-decoration: none; }
.bytes { min-width: 9ch; }
.mn { min-width: 7ch; color: var(--syn-key); }
.ops { color: var(--syn-num); flex: 1; }
.label { padding: 3px 8px 0 28px; color: var(--accent-strong); font-weight: 600; }
.pad { padding: 10px; margin: 0; }
.legend { margin: 0; font-size: 12px; display: flex; flex-wrap: wrap; gap: 8px; }
section { display: grid; gap: 4px; }
h3 { margin: 6px 0 0; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.small { margin: 0; font-size: 12px; }
.frames { margin: 0; padding-left: 2.2em; display: grid; gap: 2px; }
.frames li .muted { margin-left: 8px; }
.regs { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 4px 10px; align-items: center; }
.regs label { display: flex; align-items: center; gap: 6px; }
.regs .name { width: 3.2ch; color: var(--muted); }
.regs input { width: 100%; min-width: 0; height: 22px; padding: 0 5px; }
.regs label.changed input { color: var(--warn); }
.flags { grid-column: 1 / -1; display: flex; gap: 8px; font-family: system-ui, sans-serif; }
.flags span { color: var(--muted); }
.flags span.on { color: var(--text); font-weight: 700; }
.breaks { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
.breaks li { display: flex; align-items: center; gap: 8px; }
.breaks .icon { margin-left: auto; }
.link { border: none; background: none; padding: 0; color: var(--accent-strong); cursor: pointer; font: inherit; }
.link:hover { text-decoration: underline; }
</style>
