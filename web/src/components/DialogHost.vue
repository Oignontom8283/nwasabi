<script setup lang="ts">
// Les confirmations, saisies et choix de l'interface : des fenêtres flottantes (déplaçables, redimensionnables) qui
// bloquent le reste de la page jusqu'à la réponse.
import { nextTick, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { closeDialog, dialogState } from "../dialogs.js";
import FloatingWindow from "./FloatingWindow.vue";

const { t } = useI18n();
const text = ref("");
const checked = reactive<Record<string, boolean>>({});
const form = ref<HTMLFormElement | null>(null);

watch(
  () => dialogState.current,
  async (current) => {
    if (!current) return;
    text.value = current.value ?? "";
    for (const key of Object.keys(checked)) delete checked[key];
    for (const choice of current.choices ?? []) checked[choice.id] = choice.checked !== false;
    await nextTick();
    const input = form.value?.querySelector<HTMLInputElement>("input[type=text], input:not([type])");
    input?.focus();
    input?.select();
  },
);

function accept() {
  const current = dialogState.current;
  if (!current) return;
  if (current.kind === "prompt") closeDialog(text.value);
  else if (current.kind === "choices") closeDialog(Object.keys(checked).filter((id) => checked[id]));
  else closeDialog(true);
}

function cancel() {
  const current = dialogState.current;
  closeDialog(current?.kind === "confirm" ? false : current?.kind === "alert" ? true : null);
}
</script>

<template>
  <FloatingWindow v-if="dialogState.current" :key="dialogState.current.id" modal :width="400" :resizable="false" :title="dialogState.current.title ?? 'Nwasabi'" @close="cancel">
    <form ref="form" @submit.prevent="accept">
      <p class="message">{{ dialogState.current.message }}</p>
      <input v-if="dialogState.current.kind === 'prompt'" v-model="text" class="mono" :aria-label="dialogState.current.message" />
      <div v-if="dialogState.current.kind === 'choices'" class="choices">
        <label v-for="choice in dialogState.current.choices" :key="choice.id" class="choice">
          <input v-model="checked[choice.id]" type="checkbox" />
          <span>{{ choice.label }}<small v-if="choice.hint" class="muted"> — {{ choice.hint }}</small></span>
        </label>
      </div>
      <div class="actions">
        <button v-if="dialogState.current.kind !== 'alert'" type="button" @click="cancel">{{ dialogState.current.cancel ?? t("common.cancel") }}</button>
        <button type="submit" data-autofocus :class="['primary', { danger: dialogState.current.danger }]">
          {{ dialogState.current.accept ?? t("common.ok") }}
        </button>
      </div>
    </form>
  </FloatingWindow>
</template>

<style scoped>
.message { margin: 0 0 10px; white-space: pre-line; }
input[type="text"], input:not([type]) { width: 100%; }
.choices { display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; margin-bottom: 4px; }
.choice { display: flex; gap: 8px; align-items: flex-start; cursor: pointer; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
</style>
