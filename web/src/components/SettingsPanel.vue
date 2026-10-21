<script setup>
import { inject, onMounted, reactive, ref, watch } from "vue";

const { state, call, bump } = inject("emulator");
const values = reactive({});
const message = ref("");

const TEXT = [
  ["device_name", "Nom de l'appareil"],
  ["fcc_id", "Identifiant FCC"],
  ["serial_number", "Numéro de série (16 caractères)"],
  ["version", "Version d'Epsilon (noyau et userland)"],
  ["software_version", "Version du noyau seule"],
  ["expected_userland_version", "Version attendue du userland"],
  ["commit_hash", "Commit (7 caractères)"],
];
const NUMBERS = [
  ["pcb_version", "Version du PCB"],
  ["security_level", "Niveau de sécurité"],
  ["brightness", "Luminosité (0–255)"],
  ["clearance_level", "Niveau d'autorisation (0, 1, 2)"],
  ["compilation_flags", "Indicateurs de compilation"],
  ["last_reset_type", "Dernier redémarrage (0 matériel, 1 logiciel)"],
];

function load() {
  const r = call({ cmd: "settings_get" });
  if (!r.ok) return;
  Object.assign(values, r, { version: r.software_version, battery_level: r.battery.level, battery_voltage: r.battery.voltage, battery_charging: r.battery.charging });
}

function set(name, value) {
  const r = call({ cmd: "settings_set", name, value });
  message.value = r.ok ? "" : `${name} : ${r.error}`;
  if (r.ok) state.error = "";
  load();
  bump();
}

function setBattery() {
  set("battery", { level: Number(values.battery_level), voltage: Number(values.battery_voltage), charging: !!values.battery_charging });
}

onMounted(load);
watch(() => state.revision, load);
</script>

<template>
  <div>
    <p class="muted">
      Chaque réglage est réécrit là où l'appareil le garde (en-têtes en flash, nom, FCC…) : les
      applications le voient par leurs appels système ou en lisant la mémoire.
    </p>
    <p v-if="message" class="error">{{ message }}</p>
    <div v-for="[name, label] in TEXT" :key="name" class="field">
      <label>{{ label }}</label>
      <input v-model="values[name]" />
      <button @click="set(name, values[name])">Appliquer</button>
    </div>
    <div v-for="[name, label] in NUMBERS" :key="name" class="field">
      <label>{{ label }}</label>
      <input v-model.number="values[name]" type="number" />
      <button @click="set(name, Number(values[name]))">Appliquer</button>
    </div>
    <div class="field">
      <label>Slot actif</label>
      <select v-model="values.active_slot" @change="set('active_slot', values.active_slot)">
        <option value="a">A</option><option value="b">B</option>
      </select>
    </div>
    <div class="field">
      <label>Batterie</label>
      <span>
        niveau <input v-model.number="values.battery_level" type="number" min="0" max="5" class="short" />
        tension <input v-model.number="values.battery_voltage" type="number" step="0.1" class="short" /> V
        <label><input v-model="values.battery_charging" type="checkbox" /> en charge</label>
      </span>
      <button @click="setBattery">Appliquer</button>
    </div>
    <div class="field">
      <label>USB branché</label>
      <input v-model="values.usb_plugged" type="checkbox" @change="set('usb_plugged', !!values.usb_plugged)" />
    </div>
    <div class="field">
      <label>Mode examen (cache les applications)</label>
      <input v-model="values.exam_mode" type="checkbox" @change="set('exam_mode', !!values.exam_mode)" />
    </div>
  </div>
</template>

<style scoped>
.field { display: grid; grid-template-columns: minmax(160px, 1fr) 2fr auto; gap: 8px; align-items: center; margin-bottom: 6px; }
.field label { color: var(--muted); }
.short { width: 70px; }
</style>
