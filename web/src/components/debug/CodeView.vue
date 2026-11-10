<script setup lang="ts">
// Le code de l'application : désassemblé autour de l'instruction en cours, avec les commandes d'exécution (reprendre, pas,
// par-dessus, sortir), les points d'arrêt, la pile d'appels, les registres (modifiables en pause) et les registres de faute.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { notify, showInMemory, showWatchpoints, ui } from "../../ui.js";
import { addWatch } from "../../watchStore.js";
import { assignIfChanged, hex, hex0x, parseAddress } from "../../util.js";
import { poll } from "../../poll.js";
import { points } from "../../pointsStore.js";
import AddressLink from "../AddressLink.vue";
import { jumpKind } from "../../thumbDocs.js";
import AsmTable from "./AsmTable.vue";
import type { AsmLine } from "./asm.js";
import ContextMenu from "../ContextMenu.vue";
import type { MenuItem } from "../ContextMenu.vue";
import FaultRegisters from "../FaultRegisters.vue";
import Icon from "../Icon.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

type Line = AsmLine;
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
/** L'instruction qu'on vient de rejoindre (saut, lien, aller à…) : elle est sélectionnée, ramenée dans la vue et signalée un instant. */
const focus = ref<number | null>(null);
function go(address: number) {
  const aligned = (address & ~1) >>> 0;
  viewAt.value = aligned;
  focus.value = aligned;
}
/**
 * Le défilement libre : l'utilisateur a fait défiler le tableau au-delà de la fenêtre lue autour de l'instruction en cours. Tant qu'il
 * navigue ainsi, on ne remplace plus les lignes à chaque relevé ; la fenêtre se remet autour de l'instruction en cours dès que celle-ci
 * change (un pas, une reprise), ou qu'on demande une adresse.
 */
let browsing = false;
let loadingMore = false;
let startReached = false;
let endReached = false;
/** Nombre maximal de lignes gardées : au-delà, on retire celles du bord opposé à celui qu'on étend. */
const MAX_LINES = 900;
const CHUNK_LINES = 64;
function resetWindow() {
  browsing = false;
  startReached = false;
  endReached = false;
}
/** Lit la suite ou ce qui précède le tableau, pour pouvoir se déplacer partout comme dans l'éditeur hexadécimal. */
async function more(direction: "up" | "down") {
  const list = lines.value;
  if (loadingMore || !list.length || !state.loaded) return;
  if (direction === "up" ? startReached : endReached) return;
  loadingMore = true;
  try {
    if (direction === "down") {
      const last = list[list.length - 1] as Line;
      const d = await quiet({ cmd: "disasm", address: last.address + last.size, count: CHUNK_LINES });
      const added = d.ok ? (d.lines as Line[]) : [];
      if (!added.length) endReached = true;
      else {
        browsing = true;
        const joined = [...list, ...added];
        lines.value = joined.length > MAX_LINES ? joined.slice(joined.length - MAX_LINES) : joined;
        if (joined.length > MAX_LINES) startReached = false;
      }
    } else {
      const first = list[0] as Line;
      const d = await quiet({ cmd: "disasm", address: first.address, count: 1, before: 32 });
      const added = d.ok ? (d.lines as Line[]).filter((l) => l.address < first.address) : [];
      if (!added.length) startReached = true;
      else {
        browsing = true;
        const joined = [...added, ...list];
        lines.value = joined.length > MAX_LINES ? joined.slice(0, MAX_LINES) : joined;
        if (joined.length > MAX_LINES) endReached = false;
      }
    }
  } finally {
    loadingMore = false;
  }
}
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
const kindOf = (l: Line) => jumpKind(l.mnemonic, l.operands, l.target !== undefined);
const lineText = (l: Line) => `${hex(l.address)}  ${l.hex.padEnd(9)}  ${l.mnemonic} ${l.operands}`.trimEnd();
async function copyLines(list: Line[]) {
  if (list.length) await copyText(list.map(lineText).join("\n"));
}
const table = ref<InstanceType<typeof AsmTable> | null>(null);
async function watchRange(address: number, length: number) {
  const failure = await addWatch(emulator, address, length, "write", false);
  if (failure) message.value = failure;
  else showWatchpoints();
}
async function nopOut(line: Line) {
  // Un `nop` de la même taille : 16 bits (bf00) ou 32 bits (nop.w, f3af 8000).
  const bytes = line.size === 4 ? "aff30080" : "00bf";
  const r = await quiet({ cmd: "memory_write", address: line.address, hex: bytes });
  message.value = r.ok ? "" : String(r.error);
  emulator.bump();
  resetWindow();
  await refresh();
}
async function setPc(line: Line) {
  const r = await quiet({ cmd: "cpu_set", register: "pc", value: line.address });
  message.value = r.ok ? "" : String(r.error);
  viewAt.value = null;
  await refresh();
}
function openMenu(at: { x: number; y: number }, line: Line, selected: Line[]) {
  // Clic droit sur une sélection de plusieurs lignes : les actions de la sélection ; sinon, celles de la ligne.
  if (selected.length > 1) {
    const first = selected[0] as Line;
    const last = selected[selected.length - 1] as Line;
    const length = last.address + last.size - first.address;
    menu.value = {
      x: at.x,
      y: at.y,
      items: [
        { label: t("debug.menu.copySelection", { n: selected.length }), action: () => void copyLines(selected) },
        { label: t("link.inMemory"), action: () => showInMemory(first.address, length) },
        { label: t("link.watch"), action: () => void watchRange(first.address, length) },
        { separator: true },
        { label: t("debug.menu.clearSelection"), action: () => table.value?.clearSelection() },
      ],
    };
    return;
  }
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
    items.push({ label: t("debug.menu.gotoTarget", { target: line.target_symbol ?? hex0x(target) }), action: () => go(target) });
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
  menu.value = { x: at.x, y: at.y, items };
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
  if (cpu.value && cpu.value.pc !== next.pc) resetWindow();
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
  if (browsing) return; // l'utilisateur se déplace : ses lignes restent
  const follow = viewAt.value === null;
  // Une zone demandée : assez d'instructions pour la couvrir (deux octets au moins chacune), plus un peu de contexte.
  const count = zone.value && !follow ? Math.min(256, Math.ceil(zone.value.length / 2) + 6) : 28;
  const d = await quiet({ cmd: "disasm", address: follow ? next.pc : (viewAt.value as number), count, before: follow ? 8 : 0 });
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
watch(() => viewAt.value, resetWindow, { flush: "sync" });
watch(() => [state.revision, viewAt.value], refresh);
/** Plage d'octets à repérer dans le code (venue d'un lien « zone ») : ses instructions sont surlignées. */
const zone = ref<{ address: number; length: number } | null>(null);
watch(
  () => ui.debugGoto,
  (goto) => {
    if (!goto) return;
    go(goto.address);
    zone.value = goto.length > 1 ? { address: goto.address, length: goto.length } : null;
  },
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
  go(address);
}

// Raccourcis des débogueurs usuels : F5 reprendre ou pause, F10 par-dessus, F11 pas, Maj + F11 sortir.
function onKey(event: KeyboardEvent) {
  if (!ui.panelOpen || ui.tab !== "debug" || ui.debugView !== "code" || !state.loaded) return;
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

      <p v-if="zone" class="zone-chip">
        <span>{{ t("debug.zone", { address: hex0x(zone.address), n: zone.length }, zone.length) }}</span>
        <button class="link" @click="zone = null">{{ t("debug.zoneClear") }}</button>
      </p>
      <AsmTable @more="more"
        ref="table"
        :lines="lines"
        :pc="cpu?.pc ?? null"
        :breaks="breaks"
        :zone="zone"
        :paused="paused"
        :focus-address="focus"
        @focused="focus = null"
        @toggle-break="toggleBreak"
        @run-to="runTo"
        @follow="go"
        @memory="showInMemory"
        @menu="openMenu"
        @copy="copyLines"
      />
      <p class="legend muted">
        <button v-if="viewAt !== null" class="link" @click="(viewAt = null), (zone = null)">{{ t("debug.follow") }}</button>
        <span>{{ t("debug.legend") }}</span>
      </p>

      <section>
        <h3 class="subhead">{{ t("debug.trail.title") }}</h3>
        <p class="muted small">{{ t("debug.trail.help") }}</p>
        <div v-if="trail.length" class="trail mono">
          <div class="head muted" role="row"><span class="t-n">#</span><span class="t-addr">{{ t("debug.colAddress") }}</span><span>{{ t("debug.colInstruction") }}</span><span class="t-fn">{{ t("debug.trail.function") }}</span></div>
          <div v-for="e in trail" :key="e.n" class="t-row" role="row" :title="t('debug.trail.show')" @click="go(e.address)">
            <span class="t-n muted">{{ e.n }}</span><span class="t-addr" @click.stop><AddressLink :address="e.address" view="code" /></span><span class="t-text">{{ e.text }}</span><span class="t-fn muted">{{ e.symbol }}</span>
          </div>
        </div>
        <p v-else class="muted small">{{ t("debug.trail.none") }}</p>
        <p v-if="trail.length" class="small"><button class="link" @click="trail = []">{{ t("debug.trail.clear") }}</button></p>
      </section>

      <section>
        <h3 class="subhead">{{ t("debug.stack") }}</h3>
        <p class="muted small">{{ t("debug.stackHelp") }}</p>
        <ol v-if="paused && frames.length" class="frames mono">
          <li v-for="(frame, i) in frames" :key="i">
            <AddressLink :address="frame.address" :label="frame.symbol ?? ''" view="code" />
            <span class="muted">{{ hex0x(frame.address) }}</span>
          </li>
        </ol>
        <p v-else class="muted small">{{ t("debug.stackRunning") }}</p>
      </section>

      <section>
        <h3 class="subhead">{{ t("debug.registers") }}</h3>
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
        <h3 class="subhead">{{ t("debug.breakpoints") }}</h3>
        <p class="muted small">{{ t("debug.breakpointsHelp") }}</p>
        <ul v-if="breaks.length" class="breaks mono">
          <li v-for="address in breaks" :key="address">
            <AddressLink :address="address" view="code" />
            <button class="flat icon" :title="t('debug.removeBreak')" :aria-label="t('debug.removeBreak')" @click="toggleBreak(address)"><Icon name="trash" :size="14" /></button>
          </li>
        </ul>
        <p v-else class="muted small">{{ t("debug.noBreakpoints") }}</p>
      </section>

      <section>
        <h3 class="subhead">{{ t("debug.faultRegisters") }}</h3>
        <p class="muted small">{{ t("debug.faultRegistersHelp") }}</p>
        <FaultRegisters />
      </section>
    </template>
    <ContextMenu v-if="menu" :items="menu.items" :x="menu.x" :y="menu.y" @close="menu = null" />
  </div>
</template>

<style scoped>
.code-view { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; align-content: start; }
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
.zone-chip { margin: 0; display: flex; gap: 10px; align-items: center; font-size: 12px; color: var(--warn); }
.legend { margin: 0; font-size: 12px; display: flex; flex-wrap: wrap; gap: 8px; }
section { display: grid; gap: 4px; }
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
