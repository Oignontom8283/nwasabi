<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { computed, inject, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { GESTURES, KEYS as CALC_KEYS, KeyMap } from "@nwasabi/js";
import type { GestureName, KeyName, TouchRule, TouchSettings } from "@nwasabi/js";
import { checkedOf, download, stem, valueOf } from "../../util.js";
import { forgetAppKeymap, hasAppKeymap, saveAppKeymap } from "../../appKeymaps.js";
import Icon from "../Icon.vue";
import CalculatorSvg from "../CalculatorSvg.vue";

const { t } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const map = computed(() => (state.keymapVersion >= 0 ? emulator.keymap() : null));
const selected = ref<KeyName | "">("");
const message = ref("");
const fileInput = ref<HTMLInputElement | null>(null);
const capturing = ref<{ chord: KeyName[] } | null>(null); // on attend une touche de l'ordinateur
const byPosition = ref(true);

function apply(fn: (map: KeyMap) => KeyMap) {
  const current = map.value;
  if (!current) return;
  try {
    emulator.setKeyMap(fn(current));
    message.value = "";
  } catch (error) {
    message.value = (error as Error).message;
  }
}

const label = (id: string) => (id.startsWith("key:") ? `« ${id.slice(4) === " " ? t("keys.space") : id.slice(4)} »` : id);
const calcLabel = (name: string) => name.replace(/_/g, " ");
const sources = computed(() => (selected.value && map.value ? map.value.sourcesOf(selected.value) : null));

// ---- clavier ---------------------------------------------------------------------------------------

function capture(chord: KeyName[]) {
  capturing.value = { chord };
  addEventListener("keydown", onCaptured, true);
}
function stopCapture() {
  capturing.value = null;
  removeEventListener("keydown", onCaptured, true);
}
function onCaptured(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") return stopCapture();
  if (["Shift", "Control", "Alt", "Meta"].includes(event.key) && event.code === "") return;
  const id = byPosition.value && event.code ? event.code : `key:${event.key}`;
  const chord = capturing.value?.chord ?? [];
  stopCapture();
  apply((m) => m.withBinding(id, chord));
}
onBeforeUnmount(stopCapture);

const addKey = ref<KeyName>("up");
const bindingsOf = (id: string): readonly KeyName[] => map.value?.entries().find(([k]) => k === id)?.[1] ?? [];
function addToChord(id: string, key: KeyName) {
  const chord = [...bindingsOf(id)];
  if (!chord.includes(key)) apply((m) => m.withBinding(id, [...chord, key]));
}
function removeFromChord(id: string, key: KeyName) {
  apply((m) => m.withBinding(id, bindingsOf(id).filter((k) => k !== key)));
}

// ---- gestes ----------------------------------------------------------------------------------------

function ruleAdd() {
  const used = new Set((map.value?.rules ?? []).filter((r) => r.fingers === 1).map((r) => r.gesture));
  const gesture = GESTURES.find((g) => !used.has(g)) ?? "tap";
  apply((m) => m.withRule({ gesture, fingers: 1, keys: ["ok"] }));
}
function ruleChange(rule: TouchRule, patch: Partial<TouchRule>) {
  apply((m) => {
    const next = { gesture: rule.gesture, fingers: rule.fingers, keys: rule.keys, whileHeld: rule.whileHeld, ...patch };
    return m.withRule({ gesture: rule.gesture, fingers: rule.fingers, keys: null }).withRule({ ...next, whileHeld: next.gesture === "long_press" ? next.whileHeld : false });
  });
}
const ruleRemove = (rule: TouchRule) => apply((m) => m.withRule({ gesture: rule.gesture, fingers: rule.fingers, keys: null }));
function ruleChordAdd(rule: TouchRule, key: KeyName) {
  if (!rule.keys.includes(key)) ruleChange(rule, { keys: [...rule.keys, key] });
}
function ruleChordRemove(rule: TouchRule, key: KeyName) {
  const keys = rule.keys.filter((k) => k !== key);
  if (keys.length) ruleChange(rule, { keys });
  else ruleRemove(rule);
}

const SETTINGS: Array<[keyof TouchSettings, string, number, number, string]> = [
  ["swipeDistance", "keys.swipeDistance", 5, 1000, "px"],
  ["longPressMs", "keys.longPress", 50, 10000, "ms"],
  ["doubleTapMs", "keys.doubleTap", 50, 5000, "ms"],
  ["pressMs", "keys.pressMs", 10, 5000, "ms"],
];

/** Le menu « + » : ajoute la touche choisie à la combinaison, puis revient sur « + ». */
function chosen(event: Event, add: (key: KeyName) => void) {
  const select = event.target as HTMLSelectElement;
  if (select.value) add(select.value as KeyName);
  select.value = "";
}

// ---- fichier ---------------------------------------------------------------------------------------

async function load(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  try {
    emulator.setKeyMap(KeyMap.parse(await file.text()));
    message.value = "";
  } catch (error) {
    message.value = t("errors.keymap", { source: file.name, message: (error as Error).message });
  }
}
function exportMap() {
  const current = map.value;
  if (!current) return;
  const name = current.name || "keys";
  download(`${stem(name).replace(/[^\w.-]+/g, "_")}.nmk`, JSON.stringify(current.toNmk(), null, 2), "application/json");
}
const appName = computed(() => (emulator.state.loaded ? emulator.state.appName : ""));
const own = computed(() => hasAppKeymap(appName.value));
function keepForApp() {
  if (map.value) saveAppKeymap(appName.value, map.value);
}
const nameValue = computed(() => map.value?.name ?? "");
</script>

<template>
  <div v-if="map" class="keys">
    <div class="toolbar">
      <label class="field grow">{{ t("keys.name") }}
        <input :value="nameValue" :placeholder="t('keys.unnamed')" data-keep-keys @change="(e: Event) => apply((m) => m.withName(valueOf(e).trim() || null))" />
      </label>
      <button @click="fileInput?.click()"><Icon name="folder" /> {{ t("keys.load") }}</button>
      <button @click="exportMap"><Icon name="download" /> {{ t("keys.export") }}</button>
      <button @click="emulator.setKeyMap(KeyMap.default())"><Icon name="refresh" /> {{ t("keys.reset") }}</button>
      <input ref="fileInput" type="file" accept=".nmk,application/json" hidden @change="load" />
    </div>
    <div class="own">
      <p class="muted">{{ appName ? (own ? t("keys.ownOn", { app: appName }) : t("keys.ownOff", { app: appName })) : t("keys.ownNone") }}</p>
      <button :disabled="!appName" :title="t('keys.ownKeepHint')" @click="keepForApp">{{ t("keys.ownKeep") }}</button>
      <button v-if="own" @click="(forgetAppKeymap(appName), emulator.setKeyMap(KeyMap.default()))">{{ t("keys.ownForget") }}</button>
    </div>
    <p v-if="message" class="error" role="alert">{{ message }}</p>
    <p class="muted hint">{{ t("keys.help") }}</p>

    <div class="layout">
      <div class="calc-wrap"><CalculatorSvg mode="map" :selected="selected" @select="(k: KeyName) => (selected = selected === k ? '' : k)" /></div>

      <aside class="selected" aria-live="polite">
        <template v-if="selected && sources">
          <h2>{{ t("keys.selected", { key: calcLabel(selected) }) }}</h2>
          <ul class="sources">
            <li v-for="id in sources.keyboard" :key="id"><kbd>{{ label(id) }}</kbd>
              <button class="icon" :aria-label="t('keys.remove')" :title="t('keys.remove')" @click="removeFromChord(id, selected)"><Icon name="close" :size="12" /></button></li>
            <li v-for="rule in sources.rules" :key="rule.gesture + rule.fingers">
              {{ t(`gesture.${rule.gesture}`) }}<span v-if="rule.fingers > 1"> ×{{ rule.fingers }}</span>
              <button class="icon" :aria-label="t('keys.remove')" @click="ruleChordRemove(rule, selected)"><Icon name="close" :size="12" /></button>
            </li>
            <li v-if="!sources.keyboard.length && !sources.rules.length" class="muted">{{ t("keys.notMapped") }}</li>
          </ul>
          <div class="add">
            <button class="primary" @click="capture([selected])"><Icon name="keyboard" /> {{ t("keys.addKey") }}</button>
            <label class="inline"><input v-model="byPosition" type="checkbox" /> {{ t("keys.byPosition") }}</label>
          </div>
          <p v-if="capturing" class="capturing" role="status">{{ t("keys.pressAKey") }} <button @click="stopCapture">{{ t("common.cancel") }}</button></p>
        </template>
        <p v-else class="muted">{{ t("keys.selectHint") }}</p>
      </aside>
    </div>

    <section>
      <h2>{{ t("keys.keyboardTitle") }}</h2>
      <p class="muted hint">{{ t("keys.keyboardHelp") }}</p>
      <table class="grid-table">
        <thead><tr><th>{{ t("keys.hostKey") }}</th><th>{{ t("keys.calcKeys") }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="[id, chord] in map.entries()" :key="id">
            <td><kbd>{{ label(id) }}</kbd></td>
            <td><div class="chords">
              <span v-for="k in chord" :key="k" class="chip">{{ calcLabel(k) }}<button class="x" :aria-label="t('keys.remove')" @click="removeFromChord(id, k)"><Icon name="close" :size="10" /></button></span>
              <select class="add-select" :aria-label="t('keys.addToCombo')" @change="(e: Event) => chosen(e, (k) => addToChord(id, k))">
                <option value="">+</option>
                <option v-for="k in CALC_KEYS" :key="k" :value="k">{{ calcLabel(k) }}</option>
              </select>
            </div></td>
            <td class="end"><button class="icon" :aria-label="t('keys.removeBinding')" :title="t('keys.removeBinding')" @click="apply((m) => m.withBinding(id, null))"><Icon name="trash" /></button></td>
          </tr>
          <tr v-if="!map.entries().length"><td colspan="3" class="muted">{{ t("keys.noBindings") }}</td></tr>
        </tbody>
      </table>
      <div class="add">
        <select v-model="addKey" :aria-label="t('keys.calcKey')"><option v-for="k in CALC_KEYS" :key="k" :value="k">{{ calcLabel(k) }}</option></select>
        <button @click="capture([addKey])"><Icon name="plus" /> {{ t("keys.addBinding") }}</button>
      </div>
    </section>

    <section>
      <h2>{{ t("keys.touchTitle") }}</h2>
      <p class="muted hint">{{ t("keys.touchHelp") }}</p>
      <ul class="gestures muted">
        <li v-for="g in GESTURES" :key="g"><strong>{{ t(`gesture.${g}`) }}</strong> : {{ t(`keys.gestureInfo.${g}`) }}</li>
      </ul>
      <table class="grid-table">
        <thead><tr><th :title="t('keys.gestureHint')">{{ t("keys.gesture") }}</th><th :title="t('keys.fingersHint')">{{ t("keys.fingers") }}</th><th :title="t('keys.calcKeysHint')">{{ t("keys.calcKeys") }}</th><th :title="t('keys.heldHint')">{{ t("keys.held") }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="rule in map.rules" :key="rule.gesture + rule.fingers">
            <td>
              <select :value="rule.gesture" :aria-label="t('keys.gesture')" @change="(e: Event) => ruleChange(rule, { gesture: valueOf(e) as GestureName })">
                <option v-for="g in GESTURES" :key="g" :value="g">{{ t(`gesture.${g}`) }}</option>
              </select>
            </td>
            <td>
              <select :value="rule.fingers" :aria-label="t('keys.fingers')" @change="(e: Event) => ruleChange(rule, { fingers: Number(valueOf(e)) })">
                <option :value="1">1</option><option :value="2">2</option><option :value="3">3</option>
              </select>
            </td>
            <td><div class="chords">
              <span v-for="k in rule.keys" :key="k" class="chip">{{ calcLabel(k) }}<button class="x" :aria-label="t('keys.remove')" @click="ruleChordRemove(rule, k)"><Icon name="close" :size="10" /></button></span>
              <select class="add-select" :aria-label="t('keys.addToCombo')" @change="(e: Event) => chosen(e, (k) => ruleChordAdd(rule, k))">
                <option value="">+</option>
                <option v-for="k in CALC_KEYS" :key="k" :value="k">{{ calcLabel(k) }}</option>
              </select>
            </div></td>
            <td><input type="checkbox" :checked="rule.whileHeld" :disabled="rule.gesture !== 'long_press'" :aria-label="t('keys.held')" @change="(e: Event) => ruleChange(rule, { whileHeld: checkedOf(e) })" /></td>
            <td class="end"><button class="icon" :aria-label="t('keys.removeBinding')" @click="ruleRemove(rule)"><Icon name="trash" /></button></td>
          </tr>
          <tr v-if="!map.rules.length"><td colspan="5" class="muted">{{ t("keys.noGestures") }}</td></tr>
        </tbody>
      </table>
      <div class="add"><button @click="ruleAdd"><Icon name="plus" /> {{ t("keys.addGesture") }}</button></div>

      <h3>{{ t("keys.thresholds") }}</h3>
      <p class="muted hint">{{ t("keys.thresholdsHelp") }}</p>
      <table class="grid-table">
        <thead><tr><th>{{ t("keys.setting") }}</th><th>{{ t("keys.value") }}</th><th>{{ t("keys.meaning") }}</th></tr></thead>
        <tbody>
          <tr v-for="[prop, key, min, max, unit] in SETTINGS" :key="prop">
            <td>{{ t(key) }}</td>
            <td class="value"><input type="number" :min="min" :max="max" :value="map.settings[prop]" :aria-label="t(key)" data-keep-keys @change="(e: Event) => apply((m) => m.withSettings({ [prop]: Number(valueOf(e)) }))" /> <span class="muted">{{ unit }}</span></td>
            <td class="muted">{{ t(`${key}Info`) }}</td>
          </tr>
        </tbody>
      </table>
    </section>
  </div>
</template>

<style scoped>
.keys { display: grid; grid-template-columns: minmax(0, 1fr); gap: 20px; }
.own { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin: 8px 0; }
.own p { margin: 0; font-size: 12px; flex: 1 1 100%; }
.toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: end; }
.grow { flex: 1; min-width: 160px; }
.hint { margin: 0 0 10px; font-size: 12px; }
.layout { display: grid; grid-template-columns: minmax(180px, 280px) 1fr; gap: 16px; align-items: start; }
.calc-wrap { height: 460px; container-type: size; display: grid; place-items: center; }
.selected { min-width: 0; }
.sources { list-style: none; margin: 0 0 8px; padding: 0; display: grid; gap: 4px; }
.sources li { display: flex; align-items: center; gap: 6px; }
.add { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 10px; }
.inline { display: inline-flex; gap: 4px; font-size: 12px; align-items: center; }
.capturing { margin: 8px 0 0; padding: 6px 8px; border: 1px dashed var(--accent); border-radius: 8px; background: var(--accent-soft); }
.grid-table th { border-bottom: 1px solid var(--line); padding-bottom: 4px; }
.grid-table td { padding: 4px 8px 4px 0; border-bottom: 1px solid var(--line); vertical-align: middle; }
.chords { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
.gestures { margin: 0 0 14px; padding-left: 18px; font-size: 12px; display: grid; gap: 3px; }
.value { white-space: nowrap; }
.value input { width: 5.5em; }
.grid-table th[title] { cursor: help; }
.chip { display: inline-flex; align-items: center; gap: 2px; padding: 0 2px 0 8px; }
.x { border: none; background: transparent; padding: 0 4px; font-size: 10px; color: var(--muted); }
.add-select { width: 3.2em; padding: 0 2px; }
.end { text-align: right; }
@media (max-width: 640px) { .layout { grid-template-columns: 1fr; } }
</style>
