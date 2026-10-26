<script setup>
import { computed, onBeforeUnmount, onMounted, provide, ref } from "vue";
import { useI18n } from "vue-i18n";
import { KeyMap } from "@nwasabi/js";
import CalculatorSvg from "./components/CalculatorSvg.vue";
import DialogHost from "./components/DialogHost.vue";
import Icon from "./components/Icon.vue";
import MiniBar from "./components/MiniBar.vue";
import SidePanel from "./components/SidePanel.vue";
import TopBar from "./components/TopBar.vue";
import { alertDialog } from "./dialogs.js";
import { useEmulator } from "./emulator.js";
import { FetchError, fetchFile, fileNameOf, launchParams } from "./launch.js";
import { applyTheme, panelPixels, screenApi, ui } from "./ui.js";
import { readBytes } from "./util.js";

const { t } = useI18n();
const emulator = useEmulator();
provide("emulator", emulator);
const { state } = emulator;

const dragging = ref(false);
let dataBytes = null;
let lastApp = null;

// ---- ce que l'on charge ----------------------------------------------------------------------------

function fail(message) {
  state.error = message;
}

function loadApp(bytes, name) {
  lastApp = { bytes, name };
  emulator.load(bytes, name, dataBytes);
}

function loadData(bytes, name) {
  dataBytes = bytes;
  state.dataName = name;
  if (lastApp) emulator.load(lastApp.bytes, lastApp.name, dataBytes);
}

function loadKeymapText(text, source) {
  try {
    emulator.setKeyMap(KeyMap.parse(text));
    state.error = "";
    return true;
  } catch (error) {
    fail(t("errors.keymap", { source, message: error.message }));
    return false;
  }
}

function parseJson(text, source) {
  try {
    return JSON.parse(text);
  } catch {
    fail(t("errors.json", { source }));
    return null;
  }
}

function loadStateText(text, source) {
  const document = parseJson(text, source);
  if (!document) return false;
  const ok = emulator.restore(document);
  if (ok) {
    lastApp = null; // le dump porte son application ; « Relancer » la reprend du lecteur
    state.error = "";
  }
  return ok;
}

function loadFilesText(text, source) {
  const document = parseJson(text, source);
  if (!document) return false;
  const reply = emulator.call({ cmd: "fs_import", nfs: document, replace: true });
  if (reply.ok) {
    state.error = "";
    emulator.bump();
  }
  return reply.ok;
}

/** Charge un fichier d'après son extension. */
async function openFile(file) {
  const name = file.name;
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  if (ext === "nwa") return loadApp(await readBytes(file), name);
  if (ext === "nmk") return loadKeymapText(await file.text(), name);
  if (ext === "nds") return loadStateText(await file.text(), name);
  if (ext === "nfs") return loadFilesText(await file.text(), name);
  // L'application attend des données externes : tout autre fichier en est une.
  if (state.needsData) return loadData(await readBytes(file), name);
  fail(t("errors.unknownFile", { name }));
}

const choose = (which) => (event) => {
  const files = [...event.target.files];
  event.target.value = "";
  if (which === "data" && files[0]) readBytes(files[0]).then((bytes) => loadData(bytes, files[0].name));
  else for (const file of files) openFile(file);
};

function onDrop(event) {
  dragging.value = false;
  for (const file of event.dataTransfer.files) openFile(file);
}

// ---- adresse de la page et serveur -----------------------------------------------------------------

function describe(error) {
  if (!(error instanceof FetchError)) return error.message;
  if (error.reason === "network") return t("errors.fetchNetwork", { url: error.url });
  return t("errors.fetchHttp", { url: error.url, status: error.reason.slice(5) });
}

async function loadFromAddress() {
  const params = launchParams();
  try {
    if (params.data) loadData(new Uint8Array(await (await fetchFile(params.data)).arrayBuffer()), fileNameOf(params.data));
    if (params.nds) loadStateText(await (await fetchFile(params.nds)).text(), params.nds);
    else if (params.nwa) loadApp(new Uint8Array(await (await fetchFile(params.nwa)).arrayBuffer()), fileNameOf(params.nwa));
    if (params.nfs) loadFilesText(await (await fetchFile(params.nfs)).text(), params.nfs);
    if (params.nmk) loadKeymapText(await (await fetchFile(params.nmk)).text(), params.nmk);
  } catch (error) {
    fail(describe(error));
  }
}

/** Ce que le serveur `nwasabi` précharge (`--nwa`, `--nmk`, `--nfs`, `--nds`) ; une page hébergée ailleurs n'a rien. */
async function loadFromServer() {
  const json = async (path) => {
    try {
      const reply = await fetch(path);
      // Un autre serveur répond par la page d'accueil à toute adresse : seul `nwasabi` répond en JSON.
      return reply.ok && reply.headers.get("content-type")?.includes("application/json") ? reply : null;
    } catch {
      return null;
    }
  };
  const preload = await (await json("__nwasabi/preload"))?.json();
  if (preload) {
    state.model = preload.model;
    state.firmware = preload.firmware ?? state.firmware;
    const decode = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    dataBytes = preload.data_base64 ? decode(preload.data_base64) : null;
    emulator.call({ cmd: "new", model: preload.model });
    loadApp(decode(preload.nwa_base64), preload.name);
  }
  const nds = await json("__nwasabi/nds");
  if (nds) loadStateText(await nds.text(), t("errors.fromServer"));
  const nfs = await json("__nwasabi/nfs");
  if (nfs) loadFilesText(await nfs.text(), t("errors.fromServer"));
  const nmk = await json("__nwasabi/nmk");
  if (nmk) loadKeymapText(await nmk.text(), t("errors.fromServer"));
}

// ---- clavier global, plein écran, onglet caché -----------------------------------------------------

// En plein écran, Espace quitte et n'est jamais donné à l'application (même s'il est dans le mapping).
function onKeyCapture(event) {
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
const onResize = () => (ui.winWidth = innerWidth);
onMounted(async () => {
  applyTheme();
  addEventListener("resize", onResize);
  addEventListener("keydown", onKeyCapture, true);
  addEventListener("keyup", onKeyCapture, true);
  document.addEventListener("visibilitychange", onVisibility);
  await emulator.init();
  // Les champs de saisie, listes, éditeurs et touches de la calculatrice gardent le clavier pour eux.
  detachKeyboard = emulator.attachKeyboard(window, (e) =>
    e.target.closest?.("input, textarea, select, [contenteditable], dialog, [data-calc-key], [data-keep-keys]"),
  );
  await loadFromServer();
  await loadFromAddress();
});
onBeforeUnmount(() => {
  removeEventListener("resize", onResize);
  removeEventListener("keydown", onKeyCapture, true);
  removeEventListener("keyup", onKeyCapture, true);
  document.removeEventListener("visibilitychange", onVisibility);
  detachKeyboard();
  emulator.stop();
});

const stageStyle = computed(() => ({ right: ui.panelOpen ? `${panelPixels()}px` : "0px" }));

const input_app = ref(null);
const input_data = ref(null);
const input_keys = ref(null);
const input_state = ref(null);
const input_files = ref(null);
const pickers = { app: input_app, data: input_data, keys: input_keys, state: input_state, files: input_files };
const pick = (which) => pickers[which].value?.click();
provide("pick", pick);
</script>

<template>
  <div class="app" :class="{ dragging }" @dragover.prevent="dragging = true" @dragleave="dragging = false" @drop.prevent="onDrop">
    <TopBar @pick="pick" />
    <input ref="input_app" type="file" accept=".nwa" hidden @change="choose('app')($event)" />
    <input ref="input_data" type="file" hidden @change="choose('data')($event)" />
    <input ref="input_keys" type="file" accept=".nmk,application/json" hidden @change="choose('keys')($event)" />
    <input ref="input_state" type="file" accept=".nds,application/json" hidden @change="choose('state')($event)" />
    <input ref="input_files" type="file" accept=".nfs,application/json" hidden @change="choose('files')($event)" />

    <main class="workspace">
      <div class="stage" :style="stageStyle">
        <div class="fit"><CalculatorSvg mode="play" /></div>
      </div>
      <SidePanel v-if="ui.panelOpen" />
      <MiniBar v-else-if="ui.miniVisible" />
      <button v-else class="peek icon" :title="t('mini.showHint')" :aria-label="t('mini.showHint')" @click="ui.miniVisible = true"><Icon name="left" /></button>
    </main>

    <div v-if="state.error" class="toast" role="alert">
      <span>{{ state.error }}</span>
      <button class="icon" :aria-label="t('common.dismiss')" @click="state.error = ''"><Icon name="close" /></button>
    </div>
    <p class="sr-only" role="status" aria-live="polite">{{ state.running ? t("mini.states.running") : t("mini.states.paused") }}</p>
    <DialogHost />
  </div>
</template>

<style scoped>
.app { height: 100%; display: flex; flex-direction: column; }
.app.dragging { outline: 3px dashed var(--accent); outline-offset: -6px; }
.workspace { position: relative; flex: 1; min-height: 0; overflow: hidden; }
/* La calculatrice est centrée dans la zone qui reste : toute la largeur quand le grand panneau est fermé. */
.stage { position: absolute; left: 0; top: 0; bottom: 0; padding: 12px; }
.fit { width: 100%; height: 100%; container-type: size; display: grid; place-items: center; }
.peek { position: absolute; right: 0; top: 14px; border-radius: 8px 0 0 8px; z-index: 10; }
.toast {
  position: fixed; left: 50%; bottom: 18px; transform: translateX(-50%); max-width: min(640px, 92vw); display: flex; gap: 10px; align-items: center;
  background: var(--surface); color: var(--danger); border: 1px solid var(--danger); border-radius: 10px; padding: 8px 12px; box-shadow: var(--shadow); z-index: 50;
}
</style>
