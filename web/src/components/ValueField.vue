<script setup lang="ts">
// Un champ qui montre une valeur de la mémoire et la modifie : on tape, Entrée l'écrit, Échap annule. Tant qu'on saisit, la
// valeur relue ne remplace pas le texte. Il sert partout où une valeur se modifie sur place (recherche de valeurs, signets…) :
// le parent lit et écrit la mémoire, le champ ne s'occupe que de la saisie.
import { computed, ref } from "vue";
import { formatValue, parseValue } from "../scan.js";
import type { ScanType } from "../scan.js";

const props = defineProps<{
  /** Valeur actuelle (`null` : illisible). */
  value: number | null;
  type: ScanType;
  disabled?: boolean;
  label?: string;
}>();
const emit = defineEmits<{
  /** La valeur saisie est valide : à écrire. */
  commit: [value: number];
  /** Ce qui a été saisi n'est pas une valeur de ce type. */
  invalid: [text: string];
}>();

const draft = ref<string | null>(null);
const shown = computed(() => draft.value ?? (props.value === null ? "" : formatValue(props.value, props.type)));

function submit(field: HTMLInputElement) {
  const text = draft.value ?? field.value;
  draft.value = null;
  field.blur();
  const parsed = parseValue(text, props.type);
  if (parsed === null) emit("invalid", text);
  else emit("commit", parsed);
}
</script>

<template>
  <input
    class="value-field mono"
    :value="shown"
    :disabled="disabled"
    :aria-label="label"
    data-keep-keys
    spellcheck="false"
    @focus="draft = shown"
    @input="(e: Event) => (draft = (e.target as HTMLInputElement).value)"
    @blur="draft = null"
    @keydown.enter.prevent="(e: KeyboardEvent) => submit(e.target as HTMLInputElement)"
    @keydown.esc.prevent="(e: KeyboardEvent) => (e.target as HTMLInputElement).blur()"
  />
</template>

<style scoped>
.value-field { min-width: 0; width: 100%; height: 22px; padding: 0 5px; }
</style>
