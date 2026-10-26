<script setup>
import { nextTick, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { closeDialog, dialogState } from "../dialogs.js";

const { t } = useI18n();
const dialog = ref(null);
const text = ref("");
const checked = reactive({});
const input = ref(null);

watch(
  () => dialogState.current,
  async (current) => {
    if (!current) {
      dialog.value?.close();
      return;
    }
    text.value = current.value ?? "";
    for (const key of Object.keys(checked)) delete checked[key];
    for (const choice of current.choices ?? []) checked[choice.id] = choice.checked !== false;
    await nextTick();
    if (!dialog.value?.open) dialog.value?.showModal();
    await nextTick();
    input.value?.select?.();
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
  <dialog ref="dialog" class="dialog" @cancel.prevent="cancel" @keydown.stop>
    <form v-if="dialogState.current" method="dialog" @submit.prevent="accept">
      <h2>{{ dialogState.current.title ?? "" }}</h2>
      <p class="message">{{ dialogState.current.message }}</p>
      <input v-if="dialogState.current.kind === 'prompt'" ref="input" v-model="text" class="mono" :aria-label="dialogState.current.message" />
      <div v-if="dialogState.current.kind === 'choices'" class="choices">
        <label v-for="choice in dialogState.current.choices" :key="choice.id" class="choice">
          <input v-model="checked[choice.id]" type="checkbox" />
          <span>{{ choice.label }}<small v-if="choice.hint" class="muted"> — {{ choice.hint }}</small></span>
        </label>
      </div>
      <div class="actions">
        <button v-if="dialogState.current.kind !== 'alert'" type="button" @click="cancel">{{ t("common.cancel") }}</button>
        <button type="submit" :class="['primary', { danger: dialogState.current.danger }]">
          {{ dialogState.current.accept ?? t("common.ok") }}
        </button>
      </div>
    </form>
  </dialog>
</template>

<style scoped>
.dialog {
  border: 1px solid var(--line); border-radius: 10px; padding: 16px 18px; min-width: 320px; max-width: min(520px, 92vw);
  background: var(--surface); color: var(--text); box-shadow: var(--shadow);
}
.dialog::backdrop { background: rgb(0 0 0 / 40%); }
.message { margin: 0 0 10px; white-space: pre-line; }
input[type="text"], input:not([type]) { width: 100%; }
.choices { display: grid; gap: 6px; margin-bottom: 4px; }
.choice { display: flex; gap: 8px; align-items: flex-start; cursor: pointer; }
.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
h2:empty { display: none; }
</style>
