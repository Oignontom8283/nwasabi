<script setup>
import { computed, inject } from "vue";
import { KeyMap } from "@nwasabi/js";

const emulator = inject("emulator");
const useKeymapText = inject("useKeymapText");
const { state } = emulator;

// `keymapVersion` change à chaque nouveau mapping : c'est lui que la vue suit.
const map = computed(() => (state.keymapVersion >= 0 ? emulator.keymap() : null));

const keys = (chord) => chord.join(" + ");
const host = (id) => (id.startsWith("key:") ? `« ${id.slice(4)} »` : id);
const GESTURES = {
  tap: "Appui",
  double_tap: "Double appui",
  long_press: "Appui long",
  swipe_left: "Balayage vers la gauche",
  swipe_right: "Balayage vers la droite",
  swipe_up: "Balayage vers le haut",
  swipe_down: "Balayage vers le bas",
};

async function openMap(event) {
  const file = event.target.files[0];
  event.target.value = "";
  if (file) useKeymapText(await file.text(), file.name);
}
</script>

<template>
  <div v-if="map">
    <p>
      Mapping : <strong>{{ map.name ?? "(sans nom)" }}</strong>
      <label class="file">
        <span class="button">Charger un fichier .nmk</span>
        <input type="file" accept=".nmk,application/json" hidden @change="openMap" />
      </label>
      <button @click="emulator.setKeyMap(KeyMap.default())">Mapping par défaut</button>
    </p>
    <p class="muted">
      « Code » = position de la touche sur le clavier (identique en AZERTY et en QWERTY) ; « caractère » entre
      guillemets = ce que la touche produit. Un paramètre <code>?nmk=adresse</code> dans l'adresse de la page
      charge un mapping au démarrage.
    </p>

    <h3>Clavier de l'ordinateur</h3>
    <table>
      <thead><tr><th>Touche</th><th>Calculatrice</th></tr></thead>
      <tbody>
        <tr v-for="[id, chord] in map.entries()" :key="id">
          <td class="mono">{{ host(id) }}</td>
          <td class="mono">{{ keys(chord) }}</td>
        </tr>
      </tbody>
    </table>

    <h3>Gestes sur l'écran</h3>
    <table>
      <thead><tr><th>Geste</th><th>Doigts</th><th>Calculatrice</th></tr></thead>
      <tbody>
        <tr v-for="rule in map.rules" :key="rule.gesture + rule.fingers">
          <td>{{ GESTURES[rule.gesture] }}</td>
          <td>{{ rule.fingers }}</td>
          <td class="mono">{{ keys(rule.keys) }}{{ rule.whileHeld ? " (tant que le doigt reste posé)" : "" }}</td>
        </tr>
        <tr v-if="!map.rules.length"><td colspan="3" class="muted">Aucun geste.</td></tr>
      </tbody>
    </table>
    <p class="muted">
      Balayage : {{ map.settings.swipeDistance }} px · appui long : {{ map.settings.longPressMs }} ms · double
      appui : {{ map.settings.doubleTapMs }} ms · touche tenue : {{ map.settings.pressMs }} ms.
    </p>
  </div>
</template>

<style scoped>
table { border-collapse: collapse; width: 100%; margin-bottom: 8px; }
th, td { text-align: left; padding: 2px 10px 2px 0; border-bottom: 1px solid var(--line); }
h3 { margin: 12px 0 4px; font-size: 14px; }
.file { margin: 0 8px; }
.button { display: inline-block; border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; cursor: pointer; background: var(--key); }
</style>
