<script setup lang="ts">
// Palette de commandes (Ctrl + K) : on tape quelques lettres du nom d'une action, Entrée la lance.
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FloatingWindow from "./FloatingWindow.vue";

export interface Command {
  id: string;
  label: string;
  /** Raccourci affiché à droite, s'il y en a un. */
  shortcut?: string;
  enabled?: boolean;
  run: () => void;
}

const props = defineProps<{ commands: Command[] }>();
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();

const query = ref("");
const selected = ref(0);

/** Les lettres de la recherche doivent apparaître dans l'ordre dans le nom (sans accent ni casse). */
const plain = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
const matches = (label: string, q: string) => {
  const text = plain(label);
  let at = 0;
  for (const ch of plain(q).replace(/\s+/g, "")) {
    at = text.indexOf(ch, at);
    if (at < 0) return false;
    at++;
  }
  return true;
};
const shown = computed(() => props.commands.filter((c) => c.enabled !== false && matches(c.label, query.value)));
watch(query, () => (selected.value = 0));

function run(command: Command | undefined) {
  if (!command) return;
  emit("close");
  command.run();
}
function onKey(event: KeyboardEvent) {
  const n = shown.value.length;
  if (event.key === "ArrowDown" && n) selected.value = (selected.value + 1) % n;
  else if (event.key === "ArrowUp" && n) selected.value = (selected.value - 1 + n) % n;
  else if (event.key === "Enter") run(shown.value[selected.value]);
  else return;
  event.preventDefault();
}
</script>

<template>
  <Teleport to="body">
    <FloatingWindow modal :width="460" :resizable="false" :title="t('palette.title')" @close="emit('close')">
      <div class="palette">
        <input v-model="query" class="q" data-keep-keys data-autofocus spellcheck="false" :placeholder="t('palette.placeholder')" :aria-label="t('palette.placeholder')" @keydown="onKey" />
        <ul class="list" role="listbox">
          <li v-for="(c, i) in shown" :key="c.id" role="option" :aria-selected="i === selected" :class="{ on: i === selected }" @pointermove="selected = i" @click="run(c)">
            <span>{{ c.label }}</span>
            <kbd v-if="c.shortcut">{{ c.shortcut }}</kbd>
          </li>
          <li v-if="!shown.length" class="none muted">{{ t("palette.none") }}</li>
        </ul>
      </div>
    </FloatingWindow>
  </Teleport>
</template>

<style scoped>
.palette { display: grid; gap: 8px; }
.q { width: 100%; }
.list { list-style: none; margin: 0; padding: 0; max-height: 320px; overflow: auto; display: grid; }
.list li { display: flex; justify-content: space-between; gap: 12px; padding: 6px 10px; border-radius: 6px; cursor: pointer; }
.list li.on { background: var(--accent-soft); }
.list li.none { cursor: default; }
kbd { font: 11px ui-monospace, monospace; color: var(--muted); border: 1px solid var(--line); border-radius: 4px; padding: 0 5px; }
</style>
