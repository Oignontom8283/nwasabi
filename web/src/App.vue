<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, provide, ref, watch as watchValue } from "vue";
import { useI18n } from "vue-i18n";
import { KeyMap } from "@nwasabi/js";
import CalculatorSvg from "./components/CalculatorSvg.vue";
import AboutWindow from "./components/AboutWindow.vue";
import InfoWindow from "./components/InfoWindow.vue";
import FaultWindow from "./components/FaultWindow.vue";
import ShortcutsWindow from "./components/ShortcutsWindow.vue";
import CommandPalette from "./components/CommandPalette.vue";
import type { Command } from "./components/CommandPalette.vue";
import DialogHost from "./components/DialogHost.vue";
import Icon from "./components/Icon.vue";
import MiniBar from "./components/MiniBar.vue";
import SidePanel from "./components/SidePanel.vue";
import TopBar from "./components/TopBar.vue";
import { confirmDialog } from "./dialogs.js";
import { useEmulator } from "./emulator.js";
import { FetchError, fetchFile, fileNameOf, launchFirmware, launchModel, launchParams } from "./launch.js";
import { attachPersistence, disableKeeping, enableKeeping, keep, needsAnswer } from "./persist.js";
import { baseline, fetchPart, reload, server, stop as stopWatching, watch } from "./serverLink.js";
import type { Part } from "./serverLink.js";
import { applyTheme, isCompact, isNarrow, notice, panelPixels, recording, screenApi, TABS, ui, videoRecording } from "./ui.js";
import { loadAppKeymap } from "./appKeymaps.js";
import { recentBytes, refreshRecent, rememberApp } from "./recent.js";
import { quickLoad, quickSave, refreshQuick } from "./quicksave.js";
import { SHORTCUTS, keyed, shortcutOf } from "./shortcuts.js";
import type { ShortcutId } from "./shortcuts.js";
import type { PickKind } from "./ui.js";
import { readBytes } from "./util.js";

const { t } = useI18n();
const emulator = useEmulator();
provide("emulator", emulator);
const { state } = emulator;

let dataBytes: Uint8Array | null = null;
/** Le mapping en vigueur est celui d'une application, et le mapping général à rétablir ensuite. */
let appMap = false;
let generalMap: KeyMap | null = null;
let lastApp: { bytes: Uint8Array; name: string } | null = null;
/** Les octets à placer en flash à côté de l'application (document `.nfd`), s'il y en a. */
let flashDoc: unknown = null;

// ---- ce que l'on charge ----------------------------------------------------------------------------

function fail(message: string) {
  state.error = message;
}

async function loadApp(bytes: Uint8Array, name: string) {
  lastApp = { bytes, name };
  // Le mapping propre à l'application, s'il existe ; en le quittant on revient au mapping général.
  const own = loadAppKeymap(name);
  if (own) {
    if (!appMap) generalMap = emulator.keymap();
    appMap = true;
    emulator.setKeyMap(own);
  } else if (appMap) {
    appMap = false;
    emulator.setKeyMap(generalMap ?? KeyMap.default());
  }
  await emulator.load(bytes, name, dataBytes, flashDoc);
  if (keep.enabled) void rememberApp(name, bytes);
}

async function openRecent(name: string) {
  const bytes = await recentBytes(name);
  if (bytes) await loadApp(bytes, name);
  else fail(t("errors.recent", { name }));
}

async function loadData(bytes: Uint8Array, name: string) {
  dataBytes = bytes;
  if (lastApp) await emulator.load(lastApp.bytes, lastApp.name, dataBytes, flashDoc);
}

/** Des octets à placer en flash (fichier `.nfd`) ; l'application chargée est relancée avec eux. */
async function loadFlashText(text: string, source: string): Promise<boolean> {
  const document = parseJson(text, source);
  if (!document) return false;
  flashDoc = document;
  state.error = "";
  if (lastApp) await emulator.load(lastApp.bytes, lastApp.name, dataBytes, flashDoc);
  return !state.error;
}

function loadKeymapText(text: string, source: string): boolean {
  try {
    emulator.setKeyMap(KeyMap.parse(text));
    // Un mapping chargé à la main devient le mapping général : on ne le remplacera pas par un plus ancien en changeant d'application.
    appMap = false;
    generalMap = null;
    state.error = "";
    return true;
  } catch (error) {
    fail(t("errors.keymap", { source, message: (error as Error).message }));
    return false;
  }
}

function parseJson(text: string, source: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    fail(t("errors.json", { source }));
    return null;
  }
}

async function loadStateText(text: string, source: string): Promise<boolean> {
  const document = parseJson(text, source);
  if (!document) return false;
  const ok = (await emulator.restore(document)) ?? false;
  if (ok) {
    lastApp = null; // le dump porte son application ; « Relancer » la reprend du lecteur
    state.error = "";
  }
  return ok;
}

async function loadFilesText(text: string, source: string): Promise<boolean> {
  const document = parseJson(text, source);
  if (!document) return false;
  const reply = await emulator.call({ cmd: "fs_import", nfs: document, replace: true });
  if (reply.ok) {
    state.error = "";
    emulator.bump();
  }
  return reply.ok;
}

/** Charge un fichier d'après son extension. */
async function openFile(file: File): Promise<unknown> {
  const name = file.name;
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  if (ext === "nwa") return loadApp(await readBytes(file), name);
  if (ext === "nmk") return loadKeymapText(await file.text(), name);
  if (ext === "nds") return loadStateText(await file.text(), name);
  if (ext === "nfs") return loadFilesText(await file.text(), name);
  if (ext === "nfd") return loadFlashText(await file.text(), name);
  // L'application attend des données externes : tout autre fichier en est une.
  if (state.needsData) return loadData(await readBytes(file), name);
  fail(t("errors.unknownFile", { name }));
}

const choose = (which: string) => (event: Event) => {
  const input = event.target as HTMLInputElement;
  const files = [...(input.files ?? [])];
  input.value = "";
  const first = files[0];
  if (which === "data" && first) readBytes(first).then((bytes) => loadData(bytes, first.name));
  else for (const file of files) openFile(file);
};

function onDrop(event: DragEvent) {
  for (const file of event.dataTransfer?.files ?? []) openFile(file);
}

// ---- adresse de la page et serveur -----------------------------------------------------------------

function describe(error: unknown): string {
  if (!(error instanceof FetchError)) return (error as Error).message;
  if (error.reason === "network") return t("errors.fetchNetwork", { url: error.url });
  return t("errors.fetchHttp", { url: error.url, status: error.reason.slice(5) });
}

async function loadFromAddress() {
  const params = launchParams();
  const model = launchModel();
  const firmware = launchFirmware();
  try {
    if ((model && model !== state.model) || (firmware && firmware !== state.firmware)) {
      await emulator.changeModel(model ?? state.model, firmware ?? state.firmware);
    }
    if (params.flash) flashDoc = parseJson(await (await fetchFile(params.flash)).text(), params.flash);
    if (params.data) await loadData(new Uint8Array(await (await fetchFile(params.data)).arrayBuffer()), fileNameOf(params.data));
    if (params.nds) await loadStateText(await (await fetchFile(params.nds)).text(), params.nds);
    else if (params.nwa) await loadApp(new Uint8Array(await (await fetchFile(params.nwa)).arrayBuffer()), fileNameOf(params.nwa));
    if (params.nfs) await loadFilesText(await (await fetchFile(params.nfs)).text(), params.nfs);
    if (params.nmk) loadKeymapText(await (await fetchFile(params.nmk)).text(), params.nmk);
  } catch (error) {
    fail(describe(error));
  }
}

const decode = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

/** L'application du serveur (`--nwa`, `--data`) ; `initial` : premier chargement, qui fixe aussi le modèle. */
async function serverApp(initial: boolean) {
  const got = await fetchPart("__nwasabi/preload");
  if (got && "error" in got) return fail(got.error);
  const preload = got ? await got.reply.json() : null;
  if (!preload) return;
  if (initial) {
    await emulator.changeModel(preload.model, preload.firmware ?? state.firmware);
  }
  dataBytes = preload.data ? decode(preload.data) : null;
  await loadApp(decode(preload.nwa), preload.name);
}

/** Une partie de ce que le serveur donne : l'application, les octets en flash `.nfd`, le mapping `.nmk`, les fichiers `.nfs` ou l'état `.nds`. */
async function serverPart(part: Part) {
  if (part === "app") return serverApp(false);
  const got = await fetchPart(`__nwasabi/${part}`);
  if (!got) return;
  if ("error" in got) return fail(got.error);
  const text = await got.reply.text();
  const source = t("errors.fromServer");
  if (part === "nds") await loadStateText(text, source);
  else if (part === "flash") await loadFlashText(text, source);
  else if (part === "nfs") await loadFilesText(text, source);
  else loadKeymapText(text, source);
}

/** Ce que le serveur `nwasabi` précharge (`--nwa`, `--flash`, `--nmk`, `--nfs`, `--nds`) ; une page hébergée ailleurs n'a rien. */
async function loadFromServer() {
  await baseline();
  await serverPart("flash"); // avant l'application : elle se lance avec
  await serverApp(true);
  for (const part of ["nds", "nfs", "nmk"] as const) await serverPart(part);
  watch(serverPart, () => ui.autoReload);
}

// ---- clavier global, plein écran, onglet caché -----------------------------------------------------

// En plein écran, Espace quitte et n'est jamais donné à l'application (même s'il est dans le mapping).
/** Fait ce que dit le raccourci ; rend faux s'il n'y a rien à faire (alors la touche garde son effet habituel). */
function runShortcut(id: ShortcutId): boolean {
  const loaded = state.loaded;
  switch (id) {
    case "quickSave": void quickSave(emulator, { saved: t("quick.saved"), failed: t("quick.failed") }); return true;
    case "quickLoad": void quickLoad(emulator, { loaded: t("quick.loaded"), none: t("quick.none"), failed: t("quick.failed") }); return true;
    case "reload": if (!server.stale.length) return false; reload(); return true;
    case "toggleRun": if (!loaded) return false; void (state.running ? emulator.stop() : emulator.start()); return true;
    case "frame": if (!loaded || state.running) return false; void emulator.stepFrame(); return true;
    case "restart": if (!loaded) return false; void emulator.restart(); return true;
    case "capture": if (!loaded) return false; screenApi.capture(); return true;
    case "copy": if (!loaded) return false; screenApi.copy(); return true;
    case "gif": if (!loaded) return false; screenApi.toggleRecording(); return true;
    case "video": if (!loaded) return false; screenApi.toggleVideo(); return true;
    case "fullscreen": if (!loaded) return false; screenApi.toggleFullscreen(); return true;
    case "large": ui.large = !ui.large; return true;
    case "panel": ui.panelOpen = !ui.panelOpen; return true;
    default: return false;
  }
}

function onKeyCapture(event: KeyboardEvent) {
  // Ctrl + K (ou Cmd + K) ouvre la palette de commandes.
  if ((event.ctrlKey || event.metaKey) && event.code === "KeyK") {
    event.preventDefault();
    if (event.type === "keydown") palette.value = !palette.value;
    return;
  }
  // « ? » ouvre la liste des raccourcis (sauf dans un champ de saisie ou sur la calculatrice).
  if (event.key === "?" && event.type === "keydown" && !event.altKey && !event.ctrlKey && !(event.target as Element | null)?.closest?.("input, textarea, select, [contenteditable], [data-calc-key], [data-keep-keys]")) {
    event.preventDefault();
    shortcutsOpen.value = !shortcutsOpen.value;
    return;
  }
  // Les raccourcis Ctrl + touche (voir `shortcuts.ts`).
  const shortcut = event.type === "keydown" ? shortcutOf(event) : null;
  if (shortcut && runShortcut(shortcut)) {
    // La touche ne doit pas aussi presser la touche de la calculatrice qu'elle représente.
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  if (event.code === "Space" && document.fullscreenElement) {
    event.preventDefault();
    event.stopPropagation();
    if (event.type === "keydown") screenApi.exitFullscreen();
  }
}

// Quand l'onglet est caché, le navigateur suspend l'animation : on met la simulation en pause pour que
// l'application ne voie pas le temps passer, et on la reprend au retour (sauf si elle était déjà
// en pause). C'est une décision de cette interface, pas de la bibliothèque.
let pausedByTab = false;
function onVisibility() {
  if (document.hidden) {
    pausedByTab = state.running;
    if (pausedByTab) emulator.stop();
  } else if (pausedByTab) {
    pausedByTab = false;
    emulator.start();
  }
}

let detachKeyboard = () => {};
const onResize = () => {
  ui.winWidth = innerWidth;
  ui.winHeight = innerHeight;
};
onMounted(async () => {
  applyTheme();
  addEventListener("resize", onResize);
  addEventListener("keydown", onKeyCapture, true);
  addEventListener("keyup", onKeyCapture, true);
  document.addEventListener("visibilitychange", onVisibility);
  await emulator.init();
  attachPersistence(emulator);
  void refreshQuick();
  void refreshRecent();
  // Les champs de saisie, listes, éditeurs et touches de la calculatrice gardent le clavier pour eux.
  detachKeyboard = emulator.attachKeyboard(window, (e) =>
    !!(e.target as Element | null)?.closest?.("input, textarea, select, [contenteditable], dialog, [data-calc-key], [data-keep-keys]"),
  );
  await loadFromServer();
  await loadFromAddress();
  // Première visite : le site demande s'il peut conserver les fichiers de la calculatrice dans ce navigateur.
  if (needsAnswer()) {
    const yes = await confirmDialog(t("keepPrompt.message"), { title: t("keepPrompt.title"), accept: t("keepPrompt.accept"), cancel: t("keepPrompt.decline") });
    if (yes) await enableKeeping();
    else disableKeeping();
  }
});
onBeforeUnmount(() => {
  removeEventListener("resize", onResize);
  removeEventListener("keydown", onKeyCapture, true);
  removeEventListener("keyup", onKeyCapture, true);
  document.removeEventListener("visibilitychange", onVisibility);
  detachKeyboard();
  stopWatching();
  emulator.stop();
});

const stageStyle = computed(() => ({ right: ui.panelOpen && !isNarrow() ? `${panelPixels()}px` : "0px" }));

// Le titre de l'onglet du navigateur porte le nom de l'application : on retrouve la bonne parmi plusieurs onglets.
watchValue(
  () => (state.loaded && state.appName ? `${state.appName} — Nwasabi` : "Nwasabi"),
  (title) => (document.title = title),
  { immediate: true },
);

const palette = ref(false);
const shortcutsOpen = ref(false);
// La fenêtre de faute s'ouvre à chaque nouvel arrêt sur une faute (et pas quand on rouvre le même arrêt).
const faultOpen = ref(false);
watchValue(
  () => state.stop,
  (stop) => {
    if (stop?.kind === "fault" && !state.running) faultOpen.value = true;
    else if (stop?.kind !== "fault") faultOpen.value = false;
  },
);
const commands = computed((): Command[] => {
  const loaded = state.loaded;
  const go = (tab: string) => () => {
    ui.tab = tab;
    ui.panelOpen = true;
  };
  return [
    { id: "open-app", label: t("palette.openApp"), run: () => pick("app") },
    { id: "open-data", label: t("palette.openData"), run: () => pick("data") },
    { id: "open-state", label: t("palette.openState"), run: () => pick("state") },
    { id: "open-keys", label: t("palette.openKeys"), run: () => pick("keys") },
    { id: "open-files", label: t("palette.openFiles"), run: () => pick("files") },
    { id: "open-flash", label: t("palette.openFlash"), run: () => pick("flash") },
    { id: "pause", label: t("palette.pause"), shortcut: SHORTCUTS.toggleRun, enabled: loaded && state.running, run: () => emulator.stop() },
    { id: "resume", label: t("palette.resume"), shortcut: SHORTCUTS.toggleRun, enabled: loaded && !state.running, run: () => emulator.start() },
    { id: "restart", label: t("palette.restart"), shortcut: SHORTCUTS.restart, enabled: loaded, run: () => void emulator.restart() },
    { id: "frame", label: t("palette.frame"), shortcut: SHORTCUTS.frame, enabled: loaded && !state.running, run: () => void emulator.stepFrame() },
    { id: "capture", label: t("palette.capture"), shortcut: SHORTCUTS.capture, enabled: loaded, run: () => screenApi.capture() },
    { id: "copy", label: t("palette.copy"), shortcut: SHORTCUTS.copy, enabled: loaded, run: () => screenApi.copy() },
    { id: "gif", label: recording.on ? t("palette.gifStop") : t("palette.gif"), enabled: loaded, shortcut: SHORTCUTS.gif, run: () => screenApi.toggleRecording() },
    { id: "video", label: videoRecording.on ? t("palette.videoStop") : t("palette.video"), enabled: loaded, shortcut: SHORTCUTS.video, run: () => screenApi.toggleVideo() },
    { id: "fullscreen", label: t("palette.fullscreen"), shortcut: SHORTCUTS.fullscreen, enabled: loaded, run: () => screenApi.toggleFullscreen() },
    { id: "quick-save", label: t("palette.quickSave"), shortcut: SHORTCUTS.quickSave, enabled: loaded, run: () => void quickSave(emulator, { saved: t("quick.saved"), failed: t("quick.failed") }) },
    { id: "quick-load", label: t("palette.quickLoad"), shortcut: SHORTCUTS.quickLoad, run: () => void quickLoad(emulator, { loaded: t("quick.loaded"), none: t("quick.none"), failed: t("quick.failed") }) },
    { id: "panel", label: ui.panelOpen ? t("palette.panelClose") : t("palette.panelOpen"), shortcut: SHORTCUTS.panel, run: () => (ui.panelOpen = !ui.panelOpen) },
    ...TABS.map((id) => ({ id: `tab-${id}`, label: t("palette.goTab", { tab: t(`tabs.${id}`) }), run: go(id) })),
    { id: "shortcuts", label: t("palette.shortcuts"), shortcut: "?", run: () => (shortcutsOpen.value = true) },
    { id: "theme", label: t("palette.theme"), run: () => (ui.theme = ui.theme === "dark" ? "light" : "dark") },
  ];
});
const about = ref(false);
const info = ref(false);
const input_app = ref<HTMLInputElement | null>(null);
const input_data = ref<HTMLInputElement | null>(null);
const input_keys = ref<HTMLInputElement | null>(null);
const input_state = ref<HTMLInputElement | null>(null);
const input_files = ref<HTMLInputElement | null>(null);
const input_flash = ref<HTMLInputElement | null>(null);
const pickers: Record<PickKind, typeof input_app> = { app: input_app, data: input_data, keys: input_keys, state: input_state, files: input_files, flash: input_flash };
const pick = (which: PickKind) => pickers[which].value?.click();
provide("pick", pick);
</script>

<template>
  <div class="app" @dragover.prevent @drop.prevent="onDrop">
    <TopBar @pick="pick" @recent="openRecent" @about="about = !about" @info="info = !info" />
    <input ref="input_app" type="file" accept=".nwa" hidden @change="choose('app')($event)" />
    <input ref="input_data" type="file" hidden @change="choose('data')($event)" />
    <input ref="input_keys" type="file" accept=".nmk,application/json" hidden @change="choose('keys')($event)" />
    <input ref="input_state" type="file" accept=".nds,application/json" hidden @change="choose('state')($event)" />
    <input ref="input_files" type="file" accept=".nfs,application/json" hidden @change="choose('files')($event)" />
    <input ref="input_flash" type="file" accept=".nfd,application/json" hidden @change="choose('flash')($event)" />

    <main class="workspace">
      <div class="stage" :style="stageStyle">
        <div class="fit"><CalculatorSvg mode="play" /></div>
      </div>
      <SidePanel v-show="ui.panelOpen" />
      <template v-if="!ui.panelOpen">
        <button v-if="isCompact()" class="tools-fab" :title="keyed(t('mini.openPanel'), 'panel')" @click="ui.panelOpen = true"><Icon name="panel" :size="16" /> {{ t("mini.tools") }}</button>
        <MiniBar v-else-if="ui.miniVisible" />
        <button v-else class="peek icon" :title="t('mini.showHint')" :aria-label="t('mini.showHint')" @click="ui.miniVisible = true"><Icon name="left" /></button>
      </template>
    </main>

    <div v-if="state.error" class="toast" role="alert">
      <span>{{ state.error }}</span>
      <button class="icon" :aria-label="t('common.dismiss')" @click="state.error = ''"><Icon name="close" /></button>
    </div>
    <div v-if="notice.text" class="notice" role="status">{{ notice.text }}</div>
    <p class="sr-only" role="status" aria-live="polite">{{ state.running ? t("mini.states.running") : t("mini.states.paused") }}</p>
    <AboutWindow v-if="about" @close="about = false" />
    <InfoWindow v-if="info" @close="info = false" />
    <FaultWindow v-if="faultOpen" @close="faultOpen = false" />
    <ShortcutsWindow v-if="shortcutsOpen" @close="shortcutsOpen = false" />
    <CommandPalette v-if="palette" :commands="commands" @close="palette = false" />
    <DialogHost />
  </div>
</template>

<style scoped>
.app { height: 100%; display: flex; flex-direction: column; }
.workspace { position: relative; flex: 1; min-height: 0; overflow: hidden; }
/* La calculatrice est centrée dans la zone qui reste : toute la largeur quand le grand panneau est fermé. */
.stage { position: absolute; left: 0; top: 0; bottom: 0; padding: 10px; }
.fit { width: 100%; height: 100%; container-type: size; display: grid; place-items: center; }
.tools-fab { position: absolute; right: 12px; bottom: 12px; z-index: 10; box-shadow: var(--shadow); }
.peek { position: absolute; right: 0; top: 14px; border-radius: 8px 0 0 8px; z-index: 10; }
.notice { position: fixed; left: 50%; bottom: 18px; transform: translateX(-50%); z-index: 50; padding: 6px 14px; border-radius: 8px; background: var(--text); color: var(--surface); box-shadow: var(--shadow); font-size: 13px; }
.toast {
  position: fixed; left: 50%; bottom: 18px; transform: translateX(-50%); max-width: min(640px, 92vw); display: flex; gap: 10px; align-items: center;
  background: var(--surface); color: var(--danger); border: 1px solid var(--danger); border-radius: 10px; padding: 8px 12px; box-shadow: var(--shadow); z-index: 50;
}
</style>
