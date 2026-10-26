<script setup>
import { onMounted, onBeforeUnmount, ref, provide } from "vue";
import { useEmulator } from "./emulator.js";
import { KeyMap } from "@nwasabi/js";
import { fetchFile, fileNameOf, launchParams } from "./launch.js";
import ScreenPanel from "./components/ScreenPanel.vue";
import KeyPad from "./components/KeyPad.vue";
import FilesPanel from "./components/FilesPanel.vue";
import MemoryPanel from "./components/MemoryPanel.vue";
import StructuresPanel from "./components/StructuresPanel.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import TracePanel from "./components/TracePanel.vue";
import KeymapPanel from "./components/KeymapPanel.vue";

const emulator = useEmulator();
provide("emulator", emulator);
const { state } = emulator;

const tabs = [
  ["files", "Fichiers"],
  ["memory", "Mémoire"],
  ["structures", "Structures"],
  ["keys", "Touches"],
  ["settings", "Réglages"],
  ["trace", "Trace"],
];
const tab = ref("files");
const dragging = ref(false);
const dataName = ref("");
let dataBytes = null;
let lastApp = null;

async function readFile(file) {
  return new Uint8Array(await file.arrayBuffer());
}

async function openApp(file) {
  lastApp = { bytes: await readFile(file), name: file.name };
  emulator.load(lastApp.bytes, file.name, dataBytes);
}

async function openData(event) {
  const file = event.target.files[0];
  if (!file) return;
  dataBytes = await readFile(file);
  dataName.value = file.name;
  if (lastApp) emulator.load(lastApp.bytes, lastApp.name, dataBytes);
}

function onDrop(event) {
  dragging.value = false;
  const file = event.dataTransfer.files[0];
  if (file) openApp(file);
}

/** Applique un fichier .nmk ; une erreur de lecture est montrée avec son emplacement dans le fichier. */
function useKeymapText(text, source) {
  try {
    emulator.setKeyMap(KeyMap.parse(text));
    state.error = "";
  } catch (error) {
    state.error = `Mapping ${source} : ${error.message}`;
  }
}
provide("useKeymapText", useKeymapText);

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

/** Charge ce que demande l'adresse de la page : `?nwa=` (application) et `?nmk=` (mapping des touches). */
async function loadFromAddress() {
  const { nwa, nmk } = launchParams();
  // L'application d'abord : son chargement efface l'erreur affichée, celle d'un mapping doit rester.
  try {
    if (nwa) {
      lastApp = { bytes: new Uint8Array(await (await fetchFile(nwa)).arrayBuffer()), name: fileNameOf(nwa) };
      emulator.load(lastApp.bytes, lastApp.name, dataBytes);
    }
  } catch (error) {
    state.error = error.message;
  }
  try {
    if (nmk) useKeymapText(await (await fetchFile(nmk)).text(), nmk);
  } catch (error) {
    state.error = error.message;
  }
}

onMounted(async () => {
  document.addEventListener("visibilitychange", onVisibility);
  await emulator.init();
  // Les champs de saisie gardent le clavier pour eux.
  detachKeyboard = emulator.attachKeyboard(window, (e) => e.target.closest?.("input, textarea, select"));
  // Le serveur `nwasabi --nwa app.nwa` précharge une application ; une page hébergée ailleurs n'en a pas.
  try {
    const reply = await fetch("__nwasabi/preload");
    const preload = reply.ok ? await reply.json() : null;
    if (preload) {
      state.model = preload.model;
      state.firmware = preload.firmware ?? state.firmware;
      const decode = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      dataBytes = preload.data_base64 ? decode(preload.data_base64) : null;
      lastApp = { bytes: decode(preload.nwa_base64), name: preload.name };
      emulator.call({ cmd: "new", model: preload.model });
      emulator.load(lastApp.bytes, lastApp.name, dataBytes);
    }
  } catch {
    /* pas de serveur nwasabi : on attend un fichier */
  }
  // Le serveur `nwasabi --nmk jeu.nmk` donne aussi un mapping des touches.
  try {
    const reply = await fetch("__nwasabi/nmk");
    // Un autre serveur (Vite, hébergement statique) peut répondre par la page d'accueil à toute adresse :
    // seul le serveur nwasabi répond en JSON.
    if (reply.ok && reply.headers.get("content-type")?.includes("application/json")) {
      useKeymapText(await reply.text(), "du serveur");
    }
  } catch {
    /* idem */
  }
  await loadFromAddress();
});
onBeforeUnmount(() => {
  detachKeyboard();
  document.removeEventListener("visibilitychange", onVisibility);
  emulator.stop();
});
</script>

<template>
  <div
    class="app"
    :class="{ dragging }"
    @dragover.prevent="dragging = true"
    @dragleave="dragging = false"
    @drop.prevent="onDrop"
  >
    <header>
      <h1>nwasabi</h1>
      <label class="file">
        <span class="button primary">Ouvrir une application .nwa</span>
        <input type="file" accept=".nwa" hidden @change="(e) => e.target.files[0] && openApp(e.target.files[0])" />
      </label>
      <label class="file">
        <span class="button">Données externes{{ dataName ? ` : ${dataName}` : "" }}</span>
        <input type="file" hidden @change="openData" />
      </label>
      <label>
        Modèle
        <select :value="state.model" @change="(e) => emulator.changeModel(e.target.value)">
          <option value="n0110">N0110</option>
          <option value="n0115">N0115</option>
          <option value="n0120">N0120</option>
        </select>
      </label>
      <label title="Disposition du firmware : les applications écrites avec eadkp ne retrouvent le système de fichiers que sur celle des appareils relevés (23–24).">
        Firmware
        <select :value="state.firmware" @change="(e) => emulator.changeModel(state.model, e.target.value)">
          <option value="24">Epsilon 23–24 (appareils relevés)</option>
          <option value="26.4.0">Epsilon 26.4.0</option>
        </select>
      </label>
      <label title="Fréquence du processeur émulé : celle du modèle, une fréquence au choix, ou aucune limite (l'application reste rythmée par les 60 images par seconde de l'écran). Le nombre d'instructions réellement exécutées dépend aussi de la puissance de l'ordinateur.">
        Vitesse
        <select :value="state.speed" @change="(e) => emulator.setSpeed(e.target.value)">
          <option value="model">Modèle{{ state.speed === "model" && state.limitMhz ? ` (${state.limitMhz} MHz)` : "" }}</option>
          <option value="custom">Personnalisée</option>
          <option value="max">Maximale</option>
        </select>
      </label>
      <label v-if="state.speed === 'custom'" title="Fréquence du processeur, en MHz.">
        <input
          class="mhz"
          type="number"
          min="1"
          max="100000"
          step="1"
          :value="state.customMhz"
          @change="(e) => emulator.setCustomMhz(Number(e.target.value))"
        />
        MHz
      </label>
      <button :disabled="!state.loaded" @click="state.running ? emulator.stop() : emulator.start()">
        {{ state.running ? "Pause" : "Reprendre" }}
      </button>
      <button :disabled="!state.loaded" @click="emulator.restart()">Relancer</button>
      <span class="muted">{{ state.appName || "glisser un fichier .nwa ici" }}</span>
    </header>

    <main>
      <section class="device">
        <ScreenPanel />
        <KeyPad />
      </section>
      <section class="panel">
        <nav>
          <button v-for="[id, label] in tabs" :key="id" :class="{ primary: tab === id }" @click="tab = id">
            {{ label }}
          </button>
        </nav>
        <FilesPanel v-if="tab === 'files'" />
        <MemoryPanel v-else-if="tab === 'memory'" />
        <StructuresPanel v-else-if="tab === 'structures'" />
        <KeymapPanel v-else-if="tab === 'keys'" />
        <SettingsPanel v-else-if="tab === 'settings'" />
        <TracePanel v-else />
      </section>
    </main>
  </div>
</template>

<style scoped>
.app { min-height: 100vh; }
.app.dragging { outline: 3px dashed var(--accent); outline-offset: -8px; }
header {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  padding: 10px 16px;
  background: var(--panel);
  border-bottom: 1px solid var(--line);
}
h1 { font-size: 18px; margin: 0 8px 0 0; }
input.mhz { width: 6em; }
.button {
  display: inline-block;
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 4px 10px;
  cursor: pointer;
  background: var(--key);
}
.button.primary { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
main { display: flex; flex-wrap: wrap; gap: 16px; padding: 16px; align-items: flex-start; }
.device { display: flex; flex-direction: column; gap: 12px; }
.panel {
  flex: 1 1 420px;
  min-width: 320px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 12px;
}
nav { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 12px; }
</style>
