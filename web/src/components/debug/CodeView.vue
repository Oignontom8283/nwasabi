<script setup lang="ts">
// Le code de l'application : désassemblé autour de l'instruction en cours, avec les commandes d'exécution (reprendre, pas,
// par-dessus, sortir), les points d'arrêt, la pile d'appels, les registres (modifiables en pause) et les registres de faute.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { notify, showInMemory, ui } from "../../ui.js";
import { assignIfChanged, hex, hex0x, parseAddress, poll } from "../../util.js";
import { jumpKind, tooltip } from "../../thumbDocs.js";
import type { JumpKind } from "../../thumbDocs.js";
import { points } from "../../pointsStore.js";
import ContextMenu from "../ContextMenu.vue";
import type { MenuItem } from "../ContextMenu.vue";
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

// ---- dernières instructions exécutées (en pas à pas) --------------------------------------------------
interface Executed {
  n: number;
  address: number;
  text: string;
  symbol: string;
}
const TRAIL = 40;
const trail = ref<Executed[]>([]);
let executed = 0;
/** Note l'instruction qu'on s'apprête à exécuter (relue au besoin : la liste affichée peut avoir un instant de retard). */
async function remember(shown: Line | undefined) {
  let line = shown;
  if (!line && cpu.value) line = ((await quiet({ cmd: "disasm", address: cpu.value.pc, count: 1 })).lines as Line[] | undefined)?.[0];
  if (!line) return;
  const symbol = here.value;
  trail.value = [{ n: ++executed, address: line.address, text: `${line.mnemonic} ${line.operands}`.trim(), symbol }, ...trail.value].slice(0, TRAIL);
}
watch(() => state.appName, () => {
  trail.value = [];
  executed = 0;
});

// ---- sauts : icône, flèche au survol -------------------------------------------------------------------
const rows = ref<HTMLElement | null>(null);
const kindOf = (line: Line): JumpKind | null => jumpKind(line.mnemonic, line.operands, line.target !== undefined);
const JUMP_TITLES: Record<JumpKind, string> = { jump: "jump", branch: "branch", call: "call", return: "return", indirect: "indirect" };
interface Arrow {
  from: number;
  to: number | null;
  /** Cible hors de la liste : on la montre au bord, vers le haut ou le bas. */
  edge: "up" | "down" | null;
  label: string;
  kind: JumpKind;
}
const arrow = ref<Arrow | null>(null);
const middle = (address: number): number | null => {
  const el = rows.value?.querySelector<HTMLElement>(`[data-addr="${address}"]`);
  return el ? el.offsetTop + el.offsetHeight / 2 : null;
};
function showArrow(line: Line) {
  const kind = kindOf(line);
  const from = middle(line.address);
  if (!kind || line.target === undefined || from === null) return void (arrow.value = null);
  const to = middle(line.target);
  const label = line.target_symbol ?? hex0x(line.target);
  arrow.value = { from, to, edge: to === null ? (line.target < (lines.value[0]?.address ?? 0) ? "up" : "down") : null, label, kind };
}
const hideArrow = () => (arrow.value = null);
/** Chemin de la flèche : une accolade dans la marge de gauche, de l'instruction à sa cible. */
const arrowPath = computed(() => {
  const a = arrow.value;
  if (!a) return "";
  const x = 5;
  const right = 16;
  if (a.to === null) {
    const y = a.edge === "up" ? 4 : (rows.value?.scrollHeight ?? 0) - 4;
    return `M ${right} ${a.from} H ${x} V ${y}`;
  }
  return `M ${right} ${a.from} H ${x} V ${a.to} H ${right - 3}`;
});

// ---- clic droit ---------------------------------------------------------------------------------------
const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null);
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    notify(t("debug.menu.copied"));
  } catch {
    notify(t("debug.menu.copyFailed"));
  }
}
async function nopOut(line: Line) {
  // Un `nop` de la même taille : 16 bits (bf00) ou 32 bits (nop.w, f3af 8000).
  const bytes = line.size === 4 ? "aff30080" : "00bf";
  const r = await quiet({ cmd: "memory_write", address: line.address, hex: bytes });
  message.value = r.ok ? "" : String(r.error);
  emulator.bump();
  await refresh();
}
async function setPc(line: Line) {
  const r = await quiet({ cmd: "cpu_set", register: "pc", value: line.address });
  message.value = r.ok ? "" : String(r.error);
  viewAt.value = null;
  await refresh();
}
function openMenu(event: MouseEvent, line: Line) {
  const has = breaks.value.includes(line.address);
  const kind = kindOf(line);
  const target = line.target;
  const items: MenuItem[] = [
    { label: t("debug.menu.runTo"), disabled: !paused.value, action: () => void runTo(line.address) },
    { label: has ? t("debug.removeBreak") : t("debug.addBreak"), action: () => void toggleBreak(line.address) },
    { label: t("debug.menu.conditional"), action: () => ((points.pending = line.address), (ui.debugView = "points")) },
    { label: t("debug.menu.setPc"), disabled: !paused.value, action: () => void setPc(line) },
  ];
  if (target !== undefined) {
    items.push({ separator: true });
    items.push({ label: t("debug.menu.gotoTarget", { target: line.target_symbol ?? hex0x(target) }), action: () => (viewAt.value = target) });
    items.push({ label: t("debug.menu.breakTarget"), action: () => void toggleBreak(target) });
    items.push({ label: kind === "call" ? t("debug.menu.viewFunction") : t("debug.showInMemory"), action: () => showInMemory(target) });
  }
  items.push({ separator: true });
  items.push({ label: t("debug.menu.copyAddress"), action: () => void copyText(hex0x(line.address, 8)) });
  items.push({ label: t("debug.menu.copyInstruction"), action: () => void copyText(`${line.mnemonic} ${line.operands}`.trim()) });
  items.push({ label: t("debug.menu.copyBytes"), action: () => void copyText(line.hex) });
  items.push({ label: t("debug.menu.viewHere"), action: () => showInMemory(line.address) });
  items.push({ separator: true });
  items.push({ label: t("debug.menu.nop"), danger: true, disabled: !paused.value, action: () => void nopOut(line) });
  menu.value = { x: event.clientX, y: event.clientY, items };
}

// ---- remonter le temps ------------------------------------------------------------------------------
interface HistoryStatus {
  recording: boolean;
  checkpoints?: number;
  oldest?: number;
  newest?: number;
  current?: number;
  interval?: number;
  bytes?: number;
}
const history = ref<HistoryStatus>({ recording: false });

async function refreshHistory() {
  const r = await quiet({ cmd: "history" });
  if (r.ok) assignIfChanged(history, r as unknown as HistoryStatus);
}

async function toggleHistory() {
  await quiet({ cmd: "history", action: history.value.recording ? "stop" : "start" });
  await refreshHistory();
}

/** Après un déplacement dans le temps : écran, vues et code se mettent à jour. */
async function moved(reply: { ok: boolean; error?: string; stop?: { kind?: string } }) {
  message.value = reply.ok ? "" : String(reply.error);
  if (reply.ok && reply.stop?.kind === "history_start") message.value = t("debug.history.atStart");
  viewAt.value = null;
  await emulator.draw();
  emulator.bump();
  await refresh();
}
const stepBack = async () => moved(await quiet({ cmd: "step_back" }));
const reverseContinue = async () => moved(await quiet({ cmd: "reverse_continue" }));
const gotoCycle = async (cycles: number) => moved(await quiet({ cmd: "history_goto", cycles }));
const megaBytes = computed(() => ((history.value.bytes ?? 0) / 1048576).toFixed(1));
const reach = computed(() => Math.max(0, (history.value.current ?? 0) - (history.value.oldest ?? 0)));

async function refresh() {
  if (!state.loaded) return;
  void refreshHistory();
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
  await remember(current.value);
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
  if (instruction && /^blx?$/.test(instruction.mnemonic)) {
    await remember(instruction);
    await runTo(instruction.address + instruction.size);
  } else await step();
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
        <button class="primary" :title="`${state.running ? t('debug.pauseHint') : t('debug.resumeHint')} (F5)`" @click="state.running ? emulator.stop() : emulator.start()">
          <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("debug.pause") : t("debug.resume") }} <kbd>F5</kbd>
        </button>
        <button :disabled="!paused" :title="`${t('debug.stepHint')} (F11)`" @click="step"><Icon name="step" :size="14" /> {{ t("debug.step") }} <kbd>F11</kbd></button>
        <button :disabled="!paused" :title="`${t('debug.overHint')} (F10)`" @click="stepOver">{{ t("debug.over") }} <kbd>F10</kbd></button>
        <button :disabled="!paused" :title="`${t('debug.outHint')} (${t('debug.shiftF11')})`" @click="stepOut">{{ t("debug.out") }} <kbd>Maj F11</kbd></button>
        <form class="goto" @submit.prevent="goto">
          <input v-model="gotoText" class="mono" :placeholder="t('debug.goto')" :aria-label="t('debug.goto')" :title="t('debug.gotoHint')" data-keep-keys />
        </form>
      </div>
      <p v-if="message" class="error" role="alert">{{ message }}</p>

      <section class="time">
        <label class="record" :title="t('debug.history.hint')">
          <input type="checkbox" :checked="history.recording" @change="toggleHistory" /> {{ t("debug.history.record") }}
        </label>
        <template v-if="history.recording">
          <button :disabled="!paused" :title="t('debug.history.backHint')" @click="stepBack"><Icon name="left" :size="14" /> {{ t("debug.history.back") }}</button>
          <button :disabled="!paused" :title="t('debug.history.reverseHint')" @click="reverseContinue"><Icon name="left" :size="14" /><Icon name="left" :size="14" /> {{ t("debug.history.reverse") }}</button>
          <input
            class="slider"
            type="range"
            :min="history.oldest ?? 0"
            :max="history.newest ?? 0"
            :value="history.current ?? 0"
            :disabled="!paused || (history.newest ?? 0) <= (history.oldest ?? 0)"
            :aria-label="t('debug.history.slider')"
            :title="t('debug.history.slider')"
            @change="(e: Event) => gotoCycle(Number((e.target as HTMLInputElement).value))"
          />
          <span class="muted small stats">{{ t("debug.history.stats", { n: new Intl.NumberFormat().format(reach), mb: megaBytes }) }}</span>
        </template>
        <p v-else class="muted small">{{ t("debug.history.help") }}</p>
      </section>

      <div class="code mono" role="table" :aria-label="t('debug.code')">
        <div class="head muted" role="row">
          <span class="c-dot" :title="t('debug.colBreakHint')">{{ t("debug.colBreak") }}</span>
          <span class="c-jump" :title="t('debug.colJumpHint')">{{ t("debug.colJump") }}</span>
          <span class="c-addr">{{ t("debug.colAddress") }}</span>
          <span class="c-bytes">{{ t("debug.colBytes") }}</span>
          <span class="c-mn">{{ t("debug.colInstruction") }}</span>
          <span class="c-ops" />
          <span class="c-actions" :title="t('debug.colActionsHint')">{{ t("debug.colActions") }}</span>
        </div>
        <div ref="rows" class="rows" @mouseleave="hideArrow">
          <svg v-if="arrow" class="overlay" aria-hidden="true">
            <defs>
              <marker id="arrowhead" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" /></marker>
            </defs>
            <path :d="arrowPath" :class="['line', arrow.kind]" marker-end="url(#arrowhead)" />
            <text v-if="arrow.edge" class="edge" x="28" :y="arrow.edge === 'up' ? 14 : (rows?.scrollHeight ?? 0) - 6">{{ arrow.edge === "up" ? "↑" : "↓" }} {{ arrow.label }}</text>
          </svg>
          <template v-for="line in lines" :key="line.address">
            <div v-if="line.label" class="label">{{ line.label }}:</div>
            <div
              class="row"
              :class="{ pc: line.address === cpu?.pc, hit: breaks.includes(line.address) }"
              role="row"
              :data-addr="line.address"
              @mouseenter="showArrow(line)"
              @contextmenu.prevent="openMenu($event, line)"
            >
              <button class="dot" :title="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" :aria-label="breaks.includes(line.address) ? t('debug.removeBreak') : t('debug.addBreak')" @click="toggleBreak(line.address)">
                <span :class="{ on: breaks.includes(line.address) }" />
              </button>
              <span class="arrow" aria-hidden="true">{{ line.address === cpu?.pc ? "▶" : "" }}</span>
              <span class="jump" :class="kindOf(line) ?? ''" :title="kindOf(line) ? t(`debug.jump.${JUMP_TITLES[kindOf(line) as JumpKind]}`) : ''">
                <svg v-if="kindOf(line)" viewBox="0 0 12 12" width="14" height="14" aria-hidden="true">
                  <path v-if="kindOf(line) === 'jump' || kindOf(line) === 'branch'" :d="(line.target ?? 0) > line.address ? 'M6 1v8 M3 6.5l3 3 3-3' : 'M6 11V3 M3 5.5l3-3 3 3'" />
                  <path v-if="kindOf(line) === 'branch'" d="M1 6l1.5-1.5L4 6 2.5 7.5z" class="fill" />
                  <path v-else-if="kindOf(line) === 'call'" d="M3 2l6 4-6 4z" class="fill" />
                  <path v-else-if="kindOf(line) === 'return'" d="M10 2v4H3 M5.5 3.5L3 6l2.5 2.5" />
                  <path v-else-if="kindOf(line) === 'indirect'" d="M2 6h.01 M6 6h.01 M10 6h.01" />
                </svg>
              </span>
              <button class="addr" :title="t('debug.runTo')" :disabled="!paused" @click="runTo(line.address)">{{ hex(line.address) }}</button>
              <span class="bytes muted">{{ line.hex }}</span>
              <span class="mn" :title="tooltip(line.mnemonic, line.operands, line.target !== undefined)">{{ line.mnemonic }}</span>
              <span class="ops" :title="tooltip(line.mnemonic, line.operands, line.target !== undefined)">{{ line.operands }}<span v-if="line.target_symbol" class="muted"> &lt;{{ line.target_symbol }}&gt;</span></span>
              <span class="actions">
                <button v-if="line.target !== undefined" class="act" :title="t('debug.followTarget')" :aria-label="t('debug.followTarget')" @click="viewAt = line.target ?? null"><Icon name="right" :size="14" /></button>
                <button v-if="line.target !== undefined" class="act" :title="t('debug.showInMemory')" :aria-label="t('debug.showInMemory')" @click="showInMemory(line.target as number)"><Icon name="chip" :size="14" /></button>
              </span>
            </div>
          </template>
          <p v-if="!lines.length" class="muted pad">{{ t("debug.unreadable") }}</p>
        </div>
      </div>
      <p class="legend muted">
        <button v-if="viewAt !== null" class="link" @click="viewAt = null">{{ t("debug.follow") }}</button>
        <span>{{ t("debug.legend") }}</span>
      </p>

      <section>
        <h3>{{ t("debug.trail.title") }}</h3>
        <p class="muted small">{{ t("debug.trail.help") }}</p>
        <div v-if="trail.length" class="trail mono">
          <div class="head muted" role="row"><span class="t-n">#</span><span class="t-addr">{{ t("debug.colAddress") }}</span><span>{{ t("debug.colInstruction") }}</span><span class="t-fn">{{ t("debug.trail.function") }}</span></div>
          <div v-for="e in trail" :key="e.n" class="t-row" role="row" :title="t('debug.trail.show')" @click="viewAt = e.address">
            <span class="t-n muted">{{ e.n }}</span><span class="t-addr">{{ hex(e.address) }}</span><span class="t-text">{{ e.text }}</span><span class="t-fn muted">{{ e.symbol }}</span>
          </div>
        </div>
        <p v-else class="muted small">{{ t("debug.trail.none") }}</p>
        <p v-if="trail.length" class="small"><button class="link" @click="trail = []">{{ t("debug.trail.clear") }}</button></p>
      </section>

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
    <ContextMenu v-if="menu" :items="menu.items" :x="menu.x" :y="menu.y" @close="menu = null" />
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
.code { border: 1px solid var(--line); border-radius: 8px; overflow: auto; max-height: 440px; font-size: 12.5px; background: var(--surface); position: relative; }
.head { display: flex; align-items: center; gap: 8px; padding: 4px 8px 4px 18px; border-bottom: 1px solid var(--line); position: sticky; top: 0; z-index: 3; background: var(--surface-2); font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; white-space: nowrap; }
.c-dot { width: 38px; text-align: center; }
.c-jump { width: 18px; }
.c-addr { width: 8ch; }
.c-bytes { min-width: 9ch; }
.c-mn { min-width: 7ch; }
.c-ops { flex: 1; }
.c-actions { min-width: 62px; text-align: center; }
.rows { position: relative; min-width: max-content; }
.row { display: flex; align-items: center; gap: 8px; padding: 0 8px 0 18px; white-space: nowrap; min-height: 26px; position: relative; }
.row:hover { background: var(--accent-soft); }
.row.pc { background: color-mix(in srgb, var(--warn) 22%, transparent); font-weight: 600; }
.arrow { width: 6px; font-size: 9px; color: var(--warn); }
.dot { border: none; background: none; padding: 0 4px; width: 26px; height: 26px; display: grid; place-items: center; cursor: pointer; }
.dot span { width: 10px; height: 10px; border-radius: 50%; border: 1px solid transparent; }
.dot:hover span { border-color: var(--danger); }
.dot span.on { background: var(--danger); border-color: var(--danger); }
.jump { width: 18px; display: inline-grid; place-items: center; }
.jump svg { fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
.jump svg .fill { fill: currentColor; stroke: none; }
.jump.jump { color: var(--accent-strong); }
.jump.branch { color: var(--warn); }
.jump.call { color: var(--ok); }
.jump.return { color: var(--muted); }
.jump.indirect { color: var(--muted); }
.addr, .act { border: none; background: none; padding: 0; font: inherit; color: var(--accent-strong); cursor: pointer; }
.addr:hover { text-decoration: underline; }
.addr:disabled { color: inherit; cursor: default; text-decoration: none; }
.actions { display: inline-flex; gap: 4px; min-width: 62px; justify-content: center; }
.act { width: 28px; height: 24px; display: inline-grid; place-items: center; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); }
.act:hover { background: var(--accent-soft); border-color: var(--accent); }
.bytes { min-width: 9ch; }
.mn { min-width: 7ch; color: var(--syn-key); cursor: help; }
.ops { color: var(--syn-num); flex: 1; cursor: help; }
.label { padding: 3px 8px 0 82px; color: var(--accent-strong); font-weight: 600; }
.overlay { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; z-index: 2; overflow: visible; }
.overlay .line { fill: none; stroke: var(--accent-strong); stroke-width: 1.6; }
.overlay .line.branch { stroke: var(--warn); }
.overlay .line.call { stroke: var(--ok); }
.overlay marker path { fill: context-stroke; }
.overlay .edge { font: 11px ui-monospace, monospace; fill: var(--muted); }
.pad { padding: 10px; margin: 0; }
.trail { border: 1px solid var(--line); border-radius: 8px; overflow: auto; max-height: 260px; font-size: 12.5px; background: var(--surface); }
.trail .head { z-index: 1; }
.t-row { display: flex; gap: 8px; padding: 2px 8px; cursor: pointer; white-space: nowrap; }
.t-row:hover { background: var(--accent-soft); }
.t-n { width: 3.5ch; text-align: right; }
.t-addr { width: 8ch; color: var(--accent-strong); }
.t-text { min-width: 18ch; flex: 1; }
.t-fn { min-width: 12ch; }
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
