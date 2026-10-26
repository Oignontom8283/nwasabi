<script setup lang="ts">
// Fenêtre qui écrit une valeur en mémoire : on choisit le type et la façon de la saisir (décimal ou hexadécimal).
// Deux usages, avec la même fenêtre : remplir la sélection en répétant la valeur, ou modifier une valeur précise à partir
// de la première adresse sélectionnée (clic dans le tableau des valeurs).
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { SCAN_TYPES, SIZE, valueBytes } from "../scan.js";
import type { Base, ScanType } from "../scan.js";
import { hex, hex0x } from "../util.js";
import FloatingWindow from "./FloatingWindow.vue";

const props = defineProps<{
  mode: "fill" | "edit";
  /** Première adresse écrite. */
  address: number;
  /** Octets de la sélection (pour `fill`). */
  length: number;
  type: ScanType;
  base: Base;
  text: string;
  bigEndian: boolean;
}>();
const emit = defineEmits<{ close: []; apply: [bytes: Uint8Array] }>();
const { t } = useI18n();

const type = ref(props.type);
const base = ref(props.base);
const text = ref(props.text);

const bytes = computed(() => valueBytes(type.value, base.value, text.value, props.bigEndian));
const preview = computed(() => (bytes.value ? Array.from(bytes.value, (b) => hex(b, 2)).join(" ") : ""));
const where = computed(() =>
  props.mode === "fill"
    ? t("hex.valueFill", { address: hex0x(props.address), n: props.length }, props.length)
    : t("hex.valueEdit", { address: hex0x(props.address), n: SIZE[type.value] }, SIZE[type.value]),
);
const problem = computed(() => (text.value.trim() !== "" && !bytes.value ? t(base.value === "hex" ? "hex.valueBadHex" : "hex.valueBadDec", { type: t(`scan.types.${type.value}`) }) : ""));

function submit() {
  if (bytes.value) emit("apply", bytes.value);
}
</script>

<template>
  <!-- Hors du panneau : un conteneur à `contain: layout` ferait de lui le repère des positions fixes. -->
  <Teleport to="body">
  <FloatingWindow modal :width="400" :resizable="false" :title="mode === 'fill' ? t('hex.fillTitle') : t('hex.editTitle')" @close="emit('close')">
    <form class="form" @submit.prevent="submit">
      <p class="where">{{ where }}<span v-if="bigEndian" class="muted"> · {{ t("hex.bigEndian") }}</span></p>
      <div class="row">
        <label class="field">{{ t("scan.type") }}
          <select v-model="type">
            <option v-for="k in SCAN_TYPES" :key="k" :value="k">{{ t(`scan.types.${k}`) }}</option>
          </select>
        </label>
        <label class="field">{{ t("hex.format") }}
          <select v-model="base">
            <option value="dec">{{ t("hex.decimal") }}</option>
            <option value="hex">{{ t("hex.hexadecimal") }}</option>
          </select>
        </label>
      </div>
      <label class="field">{{ t("scan.value") }}
        <input v-model="text" class="mono" data-keep-keys data-autofocus spellcheck="false" :aria-label="t('scan.value')" />
      </label>
      <p v-if="problem" class="error" role="alert">{{ problem }}</p>
      <p v-else-if="preview" class="muted preview">{{ t("hex.bytesWritten") }} <span class="mono">{{ preview }}</span></p>
      <div class="actions">
        <button type="button" @click="emit('close')">{{ t("common.cancel") }}</button>
        <button type="submit" class="primary" :disabled="!bytes">{{ t("hex.write") }}</button>
      </div>
    </form>
  </FloatingWindow>
  </Teleport>
</template>

<style scoped>
.form { display: grid; gap: 12px; }
.where { margin: 0; }
.row { display: grid; grid-template-columns: 1fr auto; gap: 10px; }
.preview { margin: 0; font-size: 12px; }
.error { margin: 0; font-size: 12px; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
</style>
