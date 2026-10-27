<script setup lang="ts">
import { useEmu } from "../../emulator.js";
import { inject, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { confirmDialog } from "../../dialogs.js";
import { checkPersistent, clearKept, disableKeeping, enableKeeping, keep } from "../../persist.js";
import { server } from "../../serverLink.js";
import { ui } from "../../ui.js";
import { checkedOf, formatBytes, valueOf } from "../../util.js";
import type { SpeedMode } from "@nwasabi/js";

const { t, locale } = useI18n();
const emulator = useEmu();
const { state, call, quiet } = emulator;
const values = reactive<Record<string, any>>({}); // eslint-disable-line @typescript-eslint/no-explicit-any
const message = ref("");

const IDENTITY = ["device_name", "fcc_id", "serial_number", "software_version", "expected_userland_version", "commit_hash"];
const NUMBERS: Array<[string, number, number]> = [
  ["pcb_version", 0, 65535], ["security_level", 0, 255], ["clearance_level", 0, 2],
  ["compilation_flags", 0, 65535], ["last_reset_type", 0, 1],
];

async function load() {
  const r = await quiet({ cmd: "settings_get" });
  if (!r.ok) return;
  Object.assign(values, r, {
    battery_level: r.battery.level, battery_voltage: r.battery.voltage, battery_charging: r.battery.charging,
  });
}

async function set(name: string, value: unknown) {
  const r = await call({ cmd: "settings_set", name, value });
  message.value = r.ok ? "" : `${t(`settings.fields.${name}`)} : ${r.error}`;
  if (r.ok) state.error = "";
  await load();
  await emulator.bump();
}

const setBattery = () => set("battery", { level: Number(values.battery_level), voltage: Number(values.battery_voltage), charging: !!values.battery_charging });

async function toggleKeeping(event: Event) {
  if (checkedOf(event)) await enableKeeping();
  else disableKeeping();
}

async function clearStored() {
  if (await confirmDialog(t("settings.keep.confirmClear"), { danger: true, accept: t("settings.keep.clear"), title: t("settings.keep.clear") })) await clearKept();
}

onMounted(() => {
  load();
  checkPersistent();
});
watch(() => state.revision, load);
</script>

<template>
  <div class="settings">
    <p v-if="message" class="error" role="alert">{{ message }}</p>

    <section>
      <h2>{{ t("settings.page") }}</h2>
      <p class="muted help">{{ t("settings.keep.help") }}</p>
      <div class="row" :title="t('settings.keep.help')">
        <label for="keep">{{ t("settings.keep.label") }}</label>
        <input id="keep" type="checkbox" :checked="keep.enabled" @change="toggleKeeping" />
      </div>
      <template v-if="keep.enabled || keep.stored">
        <p class="muted help">
          <template v-if="keep.stored">{{ t("settings.keep.stored", { n: keep.stored.files, size: formatBytes(keep.stored.bytes, locale) }, keep.stored.files) }}</template>
          <template v-else>{{ t("settings.keep.nothing") }}</template>
          <template v-if="keep.persistent === true"> · {{ t("settings.keep.persistent") }}</template>
          <template v-else-if="keep.persistent === false"> · {{ t("settings.keep.notPersistent") }}</template>
        </p>
        <p v-if="keep.failed" class="error" role="alert">{{ t("settings.keep.failed") }}</p>
        <div class="row"><span /><span><button :disabled="!keep.stored" @click="clearStored">{{ t("settings.keep.clear") }}</button></span></div>
      </template>
      <div v-if="server.active" class="row" :title="t('settings.autoReloadHint')">
        <label for="auto">{{ t("settings.autoReload") }}</label>
        <input id="auto" v-model="ui.autoReload" type="checkbox" />
      </div>
      <p v-if="server.active" class="muted help">{{ t("settings.autoReloadHint") }}</p>
    </section>

    <section>
      <h2>{{ t("settings.screen") }}</h2>
      <div class="row" :title="t('settings.backlightHint')">
        <label for="backlight">{{ t("settings.backlight") }}</label>
        <input id="backlight" v-model="ui.backlight" type="checkbox" />
      </div>
      <p class="muted help">{{ t("settings.backlightHint") }}</p>
    </section>

    <section>
      <h2>{{ t("settings.speed") }}</h2>
      <p class="muted help">{{ t("settings.speedHelp") }}</p>
      <div class="row">
        <label :for="'speed'">{{ t("settings.speedMode") }}</label>
        <select id="speed" :value="state.speed" @change="(e: Event) => emulator.setSpeed(valueOf(e) as SpeedMode)">
          <option value="model">{{ t("settings.speedModel") }}{{ state.speed === "model" && state.limitMhz ? ` (${state.limitMhz} MHz)` : "" }}</option>
          <option value="custom">{{ t("settings.speedCustom") }}</option>
          <option value="max">{{ t("settings.speedMax") }}</option>
        </select>
      </div>
      <div v-if="state.speed === 'custom'" class="row">
        <label for="mhz">{{ t("settings.speedMhz") }}</label>
        <input id="mhz" type="number" min="1" max="100000" step="1" :value="state.customMhz" data-keep-keys @change="(e: Event) => emulator.setCustomMhz(Number(valueOf(e)))" />
      </div>
    </section>

    <section>
      <h2>{{ t("settings.identity") }}</h2>
      <p class="muted help">{{ t("settings.help") }}</p>
      <div v-for="name in IDENTITY" :key="name" class="row" :title="t(`settings.hints.${name}`)">
        <label :for="name">{{ t(`settings.fields.${name}`) }}</label>
        <input :id="name" v-model="values[name]" class="mono" data-keep-keys @change="set(name, values[name])" />
      </div>
      <div class="row">
        <label for="slot">{{ t("settings.fields.active_slot") }}</label>
        <select id="slot" v-model="values.active_slot" @change="set('active_slot', values.active_slot)"><option value="a">A</option><option value="b">B</option></select>
      </div>
    </section>

    <section>
      <h2>{{ t("settings.hardware") }}</h2>
      <div class="row" :title="t('settings.hints.brightness')">
        <label for="brightness">{{ t("settings.fields.brightness") }}</label>
        <span class="range"><input id="brightness" v-model.number="values.brightness" type="range" min="0" max="240" step="16" @change="set('brightness', values.brightness)" /> <span class="mono">{{ values.brightness }}</span></span>
      </div>
      <div class="row">
        <label for="bl">{{ t("settings.battery.level") }}</label>
        <input id="bl" v-model.number="values.battery_level" type="number" min="0" max="5" data-keep-keys @change="setBattery" />
      </div>
      <div class="row">
        <label for="bv">{{ t("settings.battery.voltage") }}</label>
        <input id="bv" v-model.number="values.battery_voltage" type="number" step="0.1" data-keep-keys @change="setBattery" />
      </div>
      <div class="row">
        <label for="bc">{{ t("settings.battery.charging") }}</label>
        <input id="bc" v-model="values.battery_charging" type="checkbox" @change="setBattery" />
      </div>
      <div class="row" :title="t('settings.hints.usb_plugged')">
        <label for="usb">{{ t("settings.fields.usb_plugged") }}</label>
        <input id="usb" v-model="values.usb_plugged" type="checkbox" @change="set('usb_plugged', !!values.usb_plugged)" />
      </div>
      <div class="row" :title="t('settings.hints.exam_mode')">
        <label for="exam">{{ t("settings.fields.exam_mode") }}</label>
        <input id="exam" v-model="values.exam_mode" type="checkbox" @change="set('exam_mode', !!values.exam_mode)" />
      </div>
    </section>

    <section>
      <h2>{{ t("settings.advanced") }}</h2>
      <div v-for="[name, min, max] in NUMBERS" :key="name" class="row" :title="t(`settings.hints.${name}`)">
        <label :for="name">{{ t(`settings.fields.${name}`) }}</label>
        <input :id="name" v-model.number="values[name]" type="number" :min="min" :max="max" data-keep-keys @change="set(name, Number(values[name]))" />
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings { display: grid; gap: 22px; }
section { display: grid; }
h2 { padding-bottom: 3px; border-bottom: 1px solid var(--line); margin-bottom: 4px; }
.help { margin: 0 0 10px; font-size: 12px; }
.row { display: grid; grid-template-columns: minmax(120px, 42%) 1fr; gap: 10px; align-items: center; min-height: 26px; }
.row label { color: var(--muted); font-size: 12.5px; }
.row input:not([type="checkbox"]):not([type="range"]), .row select { width: 100%; font-size: 13px; }
.row input[type="checkbox"] { justify-self: start; }
.range { display: flex; align-items: center; gap: 8px; }
.range input { flex: 1; }
</style>
