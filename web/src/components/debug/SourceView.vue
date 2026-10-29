<script setup lang="ts">
// Le code source de l'application, comme dans un débogueur d'ordinateur : la ligne en cours, les points d'arrêt par ligne, la pile d'appels,
// les variables locales et les expressions que l'on suit. Tout vient des informations DWARF de l'application (compilée avec `-g`) ;
// le texte des fichiers source est donné par l'utilisateur (le navigateur ne lit pas le disque).
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../../emulator.js";
import { points, refreshPoints } from "../../pointsStore.js";
import { poll } from "../../poll.js";
import { assignIfChanged, hex0x } from "../../util.js";
import { ui } from "../../ui.js";
import { addSourceFiles, evaluateExpression, readLocals, source, sourceLines } from "../../sourceStore.js";
import type { FrameInfo, VarValue } from "../../sourceStore.js";
import Icon from "../Icon.vue";
import VarTree from "./VarTree.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state, quiet } = emulator;

const frames = ref<FrameInfo[]>([]);
const params = ref<VarValue[]>([]);
const locals = ref<VarValue[]>([]);
const watched = ref<Array<VarValue & { expr: string; error?: string }>>([]);
const message = ref("");
const dragging = ref(false);
const open = reactive(new Set<string>());
/** Ce que pointent les pointeurs dépliés, par chemin. */
const derefs = reactive<Record<string, VarValue>>({});
const derefExprs: Record<string, string> = {};
const breakLines = ref<Set<number>>(new Set());
const newExpr = ref("");
const scroller = ref<HTMLElement | null>(null);
const picker = ref<HTMLInputElement | null>(null);
let stopPolling = () => {};
/** Ligne d'arrêt provisoire de « jusqu'ici » : retirée dès que l'exécution s'arrête. */
let temporary: { file: number; line: number } | null = null;

const paused = computed(() => state.loaded && !state.running);
const lines = computed(() => sourceLines(source.current));
const fileId = computed(() => source.files.find((f) => f.path === source.current)?.id ?? -1);
const codeLines = computed(() => new Set(source.codeLines[source.current ?? ""] ?? []));
const shownFrame = computed(() => frames.value[source.frame]);
/** La ligne marquée dans le fichier affiché (celle du cadre choisi). */
const hereLine = computed(() => (shownFrame.value && shownFrame.value.path === source.current ? shownFrame.value.line : undefined));

const status = computed((): { kind: string; text: string } => {
  if (!state.loaded) return { kind: "idle", text: t("debug.status.idle") };
  if (state.running) return { kind: "running", text: t("debug.status.running") };
  const stop = state.stop;
  if (stop?.kind === "fault") return { kind: "fault", text: t("debug.status.fault") };
  if (stop?.kind === "exited") return { kind: "idle", text: t("debug.status.exited", { code: Number(stop.code) }) };
  const f = frames.value[0];
  return { kind: "paused", text: f?.function ? t("source.pausedIn", { function: f.function, line: f.line ?? "?" }) : t("debug.status.paused") };
});

async function refresh() {
  if (!state.loaded || !source.available) return;
  if (state.running) return;
  const reply = await quiet({ cmd: "frames", max: 64 });
  if (reply.ok) assignIfChanged(frames, reply.frames as FrameInfo[]);
  if (source.frame >= frames.value.length) source.frame = 0;
  const vars = await readLocals(emulator, source.frame);
  assignIfChanged(params, vars.params);
  assignIfChanged(locals, vars.locals);
  const out: typeof watched.value = [];
  for (const expr of source.watches) out.push({ expr, ...(await evaluateExpression(emulator, expr, source.frame)) });
  assignIfChanged(watched, out);
  // Les pointeurs dépliés se relisent : leur cible a pu changer.
  for (const path of Object.keys(derefExprs)) {
    if (open.has(path)) await loadPointee(path, derefExprs[path] as string);
    else {
      delete derefs[path];
      delete derefExprs[path];
    }
  }
  await refreshBreakLines();
}

async function refreshBreakLines() {
  const current = new Set<number>();
  for (const b of points.breaks) {
    const at = await quiet({ cmd: "line_at", address: b.address });
    if (at.ok && at.path === source.current && typeof at.line === "number") current.add(at.line);
  }
  breakLines.value = current;
}

onMounted(() => {
  stopPolling = poll(refresh, 400);
  addEventListener("keydown", onKey);
});
onBeforeUnmount(() => {
  stopPolling();
  removeEventListener("keydown", onKey);
});
watch(() => [state.revision, source.frame, source.available], refresh);
watch(() => points.breaks, refreshBreakLines);
watch(
  () => [shownFrame.value?.path, shownFrame.value?.line],
  async () => {
    const f = shownFrame.value;
    if (f?.path && source.files.some((x) => x.path === f.path)) source.current = f.path;
    await nextTick();
    scrollToHere();
  },
);
watch(
  () => state.running,
  async (running) => {
    if (running || !temporary) return;
    await quiet({ cmd: "unbreak_line", file_id: temporary.file, line: temporary.line });
    temporary = null;
    await refreshPoints(emulator);
    await refresh();
  },
);

function scrollToHere() {
  const el = scroller.value?.querySelector<HTMLElement>(".line.here");
  el?.scrollIntoView({ block: "center" });
}

// ---- exécution ----------------------------------------------------------------------------------------
async function stepSource(mode: "into" | "over" | "out") {
  message.value = "";
  const r = await quiet({ cmd: "step_source", mode });
  if (!r.ok) message.value = String(r.error);
  source.frame = 0;
  await emulator.draw();
  emulator.bump();
  await refresh();
}

async function runToLine(line: number) {
  const file = fileId.value;
  if (file < 0) return;
  const had = breakLines.value.has(line);
  const r = await quiet({ cmd: "break_line", file_id: file, line });
  if (!r.ok) return void (message.value = String(r.error));
  if (!had) temporary = { file, line: r.line as number };
  await refreshPoints(emulator);
  await emulator.start();
}

async function toggleBreak(line: number) {
  const file = fileId.value;
  if (file < 0) return;
  const has = breakLines.value.has(line);
  const r = await quiet({ cmd: has ? "unbreak_line" : "break_line", file_id: file, line });
  if (!r.ok) message.value = String(r.error);
  else message.value = "";
  await refreshPoints(emulator);
  await refreshBreakLines();
}

function onKey(event: KeyboardEvent) {
  if (!ui.panelOpen || ui.tab !== "debug" || ui.debugView !== "source" || !state.loaded) return;
  if (event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable]")) return;
  if (event.key === "F5") void (state.running ? emulator.stop() : emulator.start());
  else if (event.key === "F10" && paused.value) void stepSource("over");
  else if (event.key === "F11" && paused.value) void stepSource(event.shiftKey ? "out" : "into");
  else return;
  event.preventDefault();
}

// ---- fichiers source ----------------------------------------------------------------------------------
async function giveFiles(list: FileList | File[] | null | undefined) {
  const files = [...(list ?? [])];
  if (!files.length) return;
  const n = await addSourceFiles(files);
  message.value = n ? "" : t("source.noMatch");
  await nextTick();
  scrollToHere();
}
const onDrop = (event: DragEvent) => {
  dragging.value = false;
  void giveFiles(event.dataTransfer?.files);
};

// ---- variables et expressions -------------------------------------------------------------------------
async function toggle(path: string, item: VarValue, expr: string) {
  if (open.has(path)) return void open.delete(path);
  open.add(path);
  if (item.pointer !== undefined && !item.children?.length) await loadPointee(path, expr);
}
/** Lit ce que pointe l'expression `expr` (déréférencement). */
async function loadPointee(path: string, expr: string) {
  derefExprs[path] = expr;
  const v = await evaluateExpression(emulator, `*(${expr})`, source.frame);
  if (v.error) delete derefs[path];
  else derefs[path] = v;
}
async function assign(item: VarValue, text: string, expr: string) {
  const value = /^-?\d+(\.\d+)?$/.test(text) ? Number(text) : text.startsWith("0x") ? parseInt(text, 16) : NaN;
  if (Number.isNaN(value)) return void (message.value = t("source.badValue"));
  const r = await quiet({ cmd: "assign", expr, value, frame: source.frame });
  message.value = r.ok ? "" : String(r.error);
  emulator.bump();
  await refresh();
}
function addWatch() {
  const expr = newExpr.value.trim();
  if (!expr) return;
  if (!source.watches.includes(expr)) source.watches.push(expr);
  newExpr.value = "";
  void refresh();
}
const removeWatch = (expr: string) => {
  source.watches = source.watches.filter((e) => e !== expr);
  void refresh();
};

// ---- survol d'un identifiant : sa valeur ----------------------------------------------------------------
const hover = ref<{ x: number; y: number; text: string } | null>(null);
let hoverTimer: ReturnType<typeof setTimeout> | undefined;
function onOver(event: MouseEvent) {
  clearTimeout(hoverTimer);
  const el = event.target instanceof Element ? event.target.closest<HTMLElement>(".id") : null;
  if (!el || !paused.value) {
    hover.value = null;
    return;
  }
  const rect = el.getBoundingClientRect();
  hoverTimer = setTimeout(async () => {
    const v = await evaluateExpression(emulator, el.textContent ?? "", source.frame);
    hover.value = v.error ? null : { x: rect.left, y: rect.bottom + 4, text: `${v.type} = ${v.value}` };
  }, 250);
}
const onLeave = () => {
  clearTimeout(hoverTimer);
  hover.value = null;
};

/** Découpe une ligne en identifiants (survolables) et autre texte. */
const KEYWORDS = new Set("if else for while do switch case break continue return goto sizeof struct union enum typedef static const volatile extern inline void int char short long float double unsigned signed fn let mut pub use mod impl match loop true false NULL".split(" "));
function tokens(line: string): Array<{ text: string; id: boolean }> {
  const out: Array<{ text: string; id: boolean }> = [];
  const re = /[A-Za-z_]\w*/g;
  let last = 0;
  for (let m = re.exec(line); m; m = re.exec(line)) {
    if (m.index > last) out.push({ text: line.slice(last, m.index), id: false });
    out.push({ text: m[0], id: !KEYWORDS.has(m[0]) });
    last = m.index + m[0].length;
  }
  if (last < line.length) out.push({ text: line.slice(last), id: false });
  return out;
}
</script>

<template>
  <div class="source-view" @dragover.prevent="dragging = true" @dragleave="dragging = false" @drop.prevent="onDrop">
    <p v-if="!state.loaded" class="empty muted">{{ t("debug.noApp") }}</p>
    <div v-else-if="!source.available" class="empty">
      <p>{{ t("source.none") }}</p>
      <p class="muted small">{{ t("source.noneHelp") }}</p>
    </div>
    <template v-else>
      <div class="status" :class="status.kind" role="status">
        <span class="dot" aria-hidden="true" />
        <span class="text">{{ status.text }}</span>
      </div>
      <div class="bar" role="toolbar" :aria-label="t('debug.controls')">
        <button class="primary" :title="`${state.running ? t('debug.pauseHint') : t('debug.resumeHint')} (F5)`" @click="state.running ? emulator.stop() : emulator.start()">
          <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("debug.pause") : t("debug.resume") }} <kbd>F5</kbd>
        </button>
        <button :disabled="!paused" :title="`${t('source.intoHint')} (F11)`" @click="stepSource('into')"><Icon name="step" :size="14" /> {{ t("source.into") }} <kbd>F11</kbd></button>
        <button :disabled="!paused" :title="`${t('source.overHint')} (F10)`" @click="stepSource('over')">{{ t("debug.over") }} <kbd>F10</kbd></button>
        <button :disabled="!paused" :title="`${t('source.outHint')} (${t('debug.shiftF11')})`" @click="stepSource('out')">{{ t("debug.out") }} <kbd>Maj F11</kbd></button>
      </div>
      <p v-if="message" class="error" role="alert">{{ message }}</p>

      <div class="files">
        <select v-model="source.current" :aria-label="t('source.file')">
          <option v-for="f in source.files" :key="f.id" :value="f.path">{{ f.path }}{{ source.texts[f.path] === undefined ? " ⚠" : "" }}</option>
        </select>
        <button :title="t('source.loadHint')" @click="picker?.click()">{{ t("source.load") }}</button>
        <input ref="picker" type="file" multiple hidden @change="(e: Event) => giveFiles((e.target as HTMLInputElement).files)" />
      </div>

      <div ref="scroller" class="listing mono" :class="{ over: dragging }" role="region" :aria-label="t('source.code')" @mouseover="onOver" @mouseleave="onLeave">
        <template v-if="lines">
          <div v-for="(text, i) in lines" :key="i" class="line" :class="{ 'has-code': codeLines.has(i + 1), here: hereLine === i + 1, brk: breakLines.has(i + 1) }">
            <button class="gutter" :disabled="!codeLines.has(i + 1)" :aria-label="breakLines.has(i + 1) ? t('debug.removeBreak') : t('debug.addBreak')" :title="codeLines.has(i + 1) ? (breakLines.has(i + 1) ? t('debug.removeBreak') : t('debug.addBreak')) : ''" @click="toggleBreak(i + 1)">
              <span v-if="breakLines.has(i + 1)" class="bp" />
            </button>
            <span class="num" :title="codeLines.has(i + 1) ? t('source.runHere') : ''" @click="codeLines.has(i + 1) && paused && runToLine(i + 1)">{{ i + 1 }}</span>
            <span class="arrow" aria-hidden="true">{{ hereLine === i + 1 ? "▶" : "" }}</span>
            <span class="text"><template v-for="(tk, j) in tokens(text)" :key="j"><span v-if="tk.id" class="id">{{ tk.text }}</span><template v-else>{{ tk.text }}</template></template></span>
          </div>
        </template>
        <div v-else class="missing">
          <p>{{ t("source.missing", { path: source.current ?? "" }) }}</p>
          <p class="muted small">{{ t("source.missingHelp") }}</p>
          <button class="primary" @click="picker?.click()">{{ t("source.load") }}</button>
        </div>
      </div>
      <p class="muted small">{{ t("source.hint") }}</p>

      <section>
        <h3 class="subhead">{{ t("source.stack") }}</h3>
        <ol v-if="paused && frames.length" class="frames mono">
          <li v-for="f in frames" :key="f.index" :class="{ chosen: source.frame === f.index }" @click="source.frame = f.index">
            <span class="fn">{{ f.function ?? f.symbol ?? hex0x(f.pc) }}</span>
            <span v-if="f.line" class="muted">{{ (f.path ?? '').split('/').pop() }}:{{ f.line }}</span>
            <span v-if="f.estimated" class="muted" :title="t('source.estimatedHint')">~</span>
          </li>
        </ol>
        <p v-else class="muted small">{{ t("debug.stackRunning") }}</p>
      </section>

      <section>
        <h3 class="subhead">{{ t("source.variables") }}</h3>
        <template v-if="paused">
          <div v-if="params.length || locals.length" class="vars">
            <VarTree v-for="v in [...params, ...locals]" :key="v.name" :item="v" :path="v.name ?? ''" :expr="v.name ?? ''" :open="open" :derefs="derefs" :editable="paused" @toggle="toggle" @assign="assign" />
          </div>
          <p v-else class="muted small">{{ t("source.noVariables") }}</p>
        </template>
        <p v-else class="muted small">{{ t("debug.stackRunning") }}</p>
      </section>

      <section>
        <h3 class="subhead">{{ t("source.expressions") }}</h3>
        <form class="add" @submit.prevent="addWatch">
          <input v-model="newExpr" class="mono" :placeholder="t('source.expressionHint')" :aria-label="t('source.expressions')" data-keep-keys />
          <button type="submit">{{ t("source.add") }}</button>
        </form>
        <div v-for="w in watched" :key="w.expr" class="watch">
          <template v-if="w.error"><span class="mono expr">{{ w.expr }}</span><span class="error small">{{ w.error }}</span></template>
          <VarTree v-else :item="{ ...w, name: w.expr }" :path="`w:${w.expr}`" :expr="w.expr" :open="open" :derefs="derefs" :editable="paused" @toggle="toggle" @assign="assign" />
          <button class="flat icon" :title="t('source.remove')" :aria-label="t('source.remove')" @click="removeWatch(w.expr)"><Icon name="trash" :size="14" /></button>
        </div>
      </section>
    </template>
    <div v-if="hover" class="tip mono" :style="{ left: hover.x + 'px', top: hover.y + 'px' }">{{ hover.text }}</div>
  </div>
</template>

<style scoped>
.source-view { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; align-content: start; }
.empty { padding: 24px 8px; text-align: center; margin: 0; }
.status { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface-2); font-size: 13px; }
.status .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--muted); flex: none; }
.status.running .dot { background: var(--ok); }
.status.paused .dot { background: var(--warn); }
.status.fault .dot { background: var(--danger); }
.bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
kbd { font: 10.5px ui-monospace, monospace; color: var(--muted); border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; margin-left: 2px; background: transparent; }
button.primary kbd { color: inherit; border-color: currentColor; opacity: 0.75; }
.files { display: flex; gap: 6px; }
.files select { flex: 1; min-width: 0; }
.listing { max-height: 340px; overflow: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); font-size: 12.5px; line-height: 1.55; tab-size: 4; }
.listing.over { outline: 2px dashed var(--accent); }
.line { display: flex; white-space: pre; }
.line.here { background: color-mix(in srgb, var(--warn) 22%, transparent); }
.gutter { width: 18px; flex: none; border: none; background: none; padding: 0; cursor: pointer; display: grid; place-items: center; }
.gutter:disabled { cursor: default; }
.line.has-code .gutter:hover:not(:disabled)::after { content: ""; width: 8px; height: 8px; border-radius: 50%; background: var(--danger); opacity: 0.35; }
.bp { width: 9px; height: 9px; border-radius: 50%; background: var(--danger); }
.num { width: 4ch; flex: none; text-align: right; color: var(--muted); padding-right: 6px; user-select: none; }
.line.has-code .num { color: var(--text); cursor: pointer; }
.arrow { width: 1.5ch; flex: none; color: var(--warn); }
.text { padding-right: 12px; }
.id:hover { background: color-mix(in srgb, var(--accent) 18%, transparent); border-radius: 2px; }
.missing { padding: 16px; text-align: center; font-family: system-ui, sans-serif; }
section { display: grid; gap: 4px; }
.small { margin: 0; font-size: 12px; }
.frames { margin: 0; padding: 0; list-style: none; display: grid; gap: 1px; }
.frames li { display: flex; gap: 8px; padding: 2px 6px; border-radius: 4px; cursor: pointer; }
.frames li:hover { background: var(--surface-2); }
.frames li.chosen { background: color-mix(in srgb, var(--accent) 16%, transparent); font-weight: 600; }
.vars { display: grid; gap: 1px; }
.add { display: flex; gap: 6px; }
.add input { flex: 1; min-width: 0; }
.watch { display: flex; align-items: baseline; gap: 6px; }
.watch :deep(.var) { flex: 1; min-width: 0; }
.expr { font-weight: 600; }
.tip { position: fixed; z-index: 50; background: var(--surface); border: 1px solid var(--line); border-radius: 6px; padding: 3px 8px; font-size: 12px; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.2); pointer-events: none; max-width: 60vw; overflow: hidden; text-overflow: ellipsis; }
</style>
