<script setup lang="ts">
// Fenêtre qui enregistre une zone de mémoire dans un fichier : en octets bruts, ou en texte hexadécimal.
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useEmu } from "../emulator.js";
import { notify } from "../ui.js";
import { download, hex, hex0x, hexToBytes, parseAddress, stem } from "../util.js";
import FloatingWindow from "./FloatingWindow.vue";

const props = defineProps<{ address: number; length: number }>();
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
const emulator = useEmu();

const MAX = 16 * 1024 * 1024;
const CHUNK = 1 << 20;
const start = ref(hex0x(props.address));
const length = ref(String(props.length));
const format = ref<"bin" | "hex">("bin");
const busy = ref(false);
const error = ref("");

const range = computed(() => {
  const address = parseAddress(start.value);
  const size = parseAddress(length.value);
  return address === undefined || size === undefined || size < 1 || address + size > 0x1_0000_0000 ? null : { address, size };
});

async function save() {
  const r = range.value;
  if (!r) return;
  if (r.size > MAX) return void (error.value = t("export.tooBig"));
  busy.value = true;
  error.value = "";
  const out = new Uint8Array(r.size);
  for (let done = 0; done < r.size; done += CHUNK) {
    const reply = await emulator.quiet({ cmd: "memory_read", address: r.address + done, length: Math.min(CHUNK, r.size - done) });
    if (!reply.ok) {
      busy.value = false;
      return void (error.value = String(reply.error));
    }
    out.set(hexToBytes(String(reply.hex)), done);
  }
  const base = `${stem(emulator.state.appName || "nwasabi")}-${hex(r.address)}`;
  if (format.value === "bin") download(`${base}.bin`, out);
  else {
    const lines: string[] = [];
    for (let i = 0; i < out.length; i += 16) {
      const row = Array.from(out.subarray(i, i + 16), (b) => hex(b, 2)).join(" ");
      lines.push(`${hex(r.address + i)}  ${row}`);
    }
    download(`${base}.txt`, lines.join("\n") + "\n", "text/plain");
  }
  busy.value = false;
  notify(t("export.done", { n: r.size }, r.size));
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <FloatingWindow modal :width="380" :resizable="false" :title="t('export.title')" @close="emit('close')">
      <form class="form" @submit.prevent="save">
        <div class="row">
          <label class="field">{{ t("export.address") }}<input v-model="start" class="mono" data-keep-keys data-autofocus spellcheck="false" /></label>
          <label class="field">{{ t("export.length") }}<input v-model="length" class="mono" data-keep-keys spellcheck="false" /></label>
        </div>
        <label class="field">{{ t("export.format") }}
          <select v-model="format">
            <option value="bin">{{ t("export.bin") }}</option>
            <option value="hex">{{ t("export.hex") }}</option>
          </select>
        </label>
        <p v-if="!range" class="error" role="alert">{{ t("export.invalid") }}</p>
        <p v-else-if="error" class="error" role="alert">{{ error }}</p>
        <div class="actions">
          <button type="button" @click="emit('close')">{{ t("common.cancel") }}</button>
          <button type="submit" class="primary" :disabled="!range || busy">{{ t("export.save") }}</button>
        </div>
      </form>
    </FloatingWindow>
  </Teleport>
</template>

<style scoped>
.form { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; }
.row { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.error { margin: 0; font-size: 12px; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
</style>
